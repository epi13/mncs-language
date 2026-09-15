#!/usr/bin/env python3
"""Generate small, deterministic bindings for the MNCS typed-call ABI.

The compiler owns callable metadata and the interface identity.  This tool
only turns that metadata into host-language ergonomics; it does not invent a
second schema, infer topology, or accept executable code from ABI data.

The generated Python binding uses the existing ``mncs execute`` wire entry
point.  The generated Rust binding uses ``mncs-embed``.  Both submit the
interface identity with every typed call, making an old binding fail closed
when the callable contract changes.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import keyword
import re
from pathlib import Path
from typing import Any


GENERATOR_VERSION = "mncs-host-bindings/0.1"


def binding_content_identity(abi: dict[str, Any], language: str) -> str:
    """Identity of the deterministic binding material, not a file timestamp."""

    material = {
        "binding_language": language,
        "generator_version": GENERATOR_VERSION,
        "module_identity": abi["module"],
        "interface_identity": abi["interface_identity"],
        "typed_call_schema_version": abi.get("typed_call_schema_version", "mncs.typed-call/1"),
        "functions": abi["functions"],
        "composites": abi["composites"],
    }
    encoded = json.dumps(material, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def load_abi(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    required = ("schema_version", "interface_identity", "module", "functions", "composites")
    if not isinstance(value, dict) or any(not value.get(key) for key in required):
        raise SystemExit(f"ABI document {path} is missing required metadata")
    if value["schema_version"] != "0.1":
        raise SystemExit(f"unsupported ABI schema: {value['schema_version']!r}")
    return value


def short_name(value: str) -> str:
    return value.rsplit("::", 1)[-1]


def safe_identifier(value: str) -> str:
    value = re.sub(r"[^A-Za-z0-9_]", "_", value)
    if not value or value[0].isdigit() or keyword.iskeyword(value):
        value = f"_{value}"
    return value


def unique_composites(abi: dict[str, Any]) -> dict[str, dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    for key, contract in abi["composites"].items():
        if not isinstance(contract, dict):
            continue
        shape = contract.get("finite") or contract.get("record")
        if not isinstance(shape, dict):
            continue
        name = shape.get("name") or short_name(key)
        if not isinstance(name, str) or not name:
            continue
        # The ABI includes both nominal names and fully-qualified identity
        # keys. The first shape is sufficient because the identity itself is
        # carried in the shape and must agree at runtime.
        result.setdefault(name, contract)
    return dict(sorted(result.items()))


def shape(contract: Any) -> dict[str, Any]:
    if not isinstance(contract, dict):
        return {}
    for kind in ("finite", "record", "sequence", "vector", "mask", "scalar"):
        if kind in contract and isinstance(contract[kind], dict):
            return {kind: contract[kind]}
    return contract


def type_name(contract: Any) -> str | None:
    value = shape(contract)
    if "boolean" in value:
        return "bool"
    if "integer" in value:
        return "int"
    if "float" in value:
        return "float"
    if "finite" in value:
        return value["finite"].get("name")
    if "record" in value:
        return value["record"].get("name")
    return None


def python_descriptor(contract: Any) -> str:
    if isinstance(contract, str):
        if contract == "bool":
            return "bool"
        if contract.startswith(("i", "u")) and contract[1:].isdigit():
            return "int"
        if contract.startswith("f") and contract[1:].isdigit():
            return "float"
        if contract:
            return f"record:{short_name(contract)}"
        return "unknown"
    value = shape(contract)
    if "boolean" in value:
        return "bool"
    if "integer" in value:
        return "int"
    if "float" in value:
        return "float"
    if "byte" in value:
        return "int"
    if "finite" in value:
        return f"finite:{value['finite'].get('name', '')}"
    if "record" in value:
        return f"record:{value['record'].get('name', '')}"
    if "sequence" in value:
        sequence = value["sequence"]
        element = sequence.get("element")
        bound = sequence.get("bound", {}).get("exact")
        return f"sequence:{python_descriptor(element)}:{bound if bound is not None else ''}"
    if "vector" in value:
        vector = value["vector"]
        return f"vector:{python_descriptor(vector.get('element'))}:{vector.get('lanes', '')}"
    if "mask" in value:
        return f"mask:{value['mask'].get('lanes', '')}"
    if "scalar" in value:
        semantic = value["scalar"].get("semantic_type")
        if semantic == "bool":
            return "bool"
        if isinstance(semantic, dict) and "integer" in semantic:
            return "int"
        if isinstance(semantic, dict) and "float" in semantic:
            return "float"
        return "scalar"
    return "unknown"


def python_annotation(descriptor: str) -> str:
    if descriptor == "bool":
        return "bool"
    if descriptor == "int":
        return "int"
    if descriptor == "float":
        return "float"
    if descriptor.startswith("finite:") or descriptor.startswith("record:"):
        return safe_identifier(descriptor.split(":", 1)[1])
    if descriptor.startswith(("sequence:", "vector:")):
        return "tuple[Any, ...]"
    if descriptor.startswith("mask:"):
        return "tuple[bool, ...]"
    return "Any"


def python_binding(abi: dict[str, Any]) -> str:
    composites = unique_composites(abi)
    finite_types: dict[str, dict[str, Any]] = {}
    record_types: dict[str, dict[str, Any]] = {}
    for name, contract in composites.items():
        if "finite" in contract:
            finite_types[name] = contract["finite"]
        elif "record" in contract:
            record_types[name] = contract["record"]

    out: list[str] = [
        '"""Generated by mncs-language/scripts/generate_host_bindings.py. Do not edit."""',
        "from __future__ import annotations",
        "",
        "import json",
        "import os",
        "import subprocess",
        "import tempfile",
        "from dataclasses import dataclass",
        "from enum import Enum",
        "from pathlib import Path",
        "from typing import Any",
        "",
        f"GENERATOR_VERSION = {GENERATOR_VERSION!r}",
        f"MODULE_IDENTITY = {abi['module']!r}",
        f"INTERFACE_IDENTITY = {abi['interface_identity']!r}",
        f"TYPED_CALL_SCHEMA_VERSION = {abi.get('typed_call_schema_version', 'mncs.typed-call/1')!r}",
        f"BINDING_CONTENT_IDENTITY = {binding_content_identity(abi, 'python')!r}",
        "",
        "class BindingError(RuntimeError):",
        "    pass",
        "",
        "def _finite_payload(type_name: str, variant: str) -> dict[str, Any]:",
        "    return {'finite': {'type': type_name, 'variant': variant}}",
        "",
        "def _fields(value: Any) -> dict[str, Any]:",
        "    record = value.get('record') if isinstance(value, dict) else None",
        "    pairs = record.get('fields') if isinstance(record, dict) else None",
        "    if isinstance(pairs, dict):",
        "        return pairs",
        "    if isinstance(pairs, list):",
        "        return {pair[0]: pair[1] for pair in pairs if isinstance(pair, list) and len(pair) == 2 and isinstance(pair[0], str)}",
        "    raise BindingError('returned value is not a typed record')",
        "",
        "def _variant(value: Any) -> str:",
        "    finite = value.get('finite') if isinstance(value, dict) else None",
        "    if not isinstance(finite, dict):",
        "        raise BindingError('returned value is not a typed finite value')",
        "    identity = finite.get('variant_identity')",
        "    if isinstance(identity, str) and '::' in identity:",
        "        return identity.rsplit('::', 1)[-1]",
        "    variant = finite.get('variant')",
        "    if isinstance(variant, str) and variant:",
        "        return variant",
        "    raise BindingError('typed finite value has no variant identity')",
        "",
        "def _encode(value: Any) -> Any:",
        "    if isinstance(value, Enum):",
        "        return _finite_payload(value.__class__.__name__, value.value)",
        "    if hasattr(value, 'to_host_value'):",
        "        return value.to_host_value()",
        "    if isinstance(value, bool):",
        "        return {'boolean': {'value': value}}",
        "    if isinstance(value, int):",
        "        return {'integer': {'value': value}}",
        "    if isinstance(value, float):",
        "        return {'float': {'value': value}}",
        "    if isinstance(value, (tuple, list)):",
        "        return {'sequence': {'values': [_encode(item) for item in value]}}",
        "    return value",
        "",
        "def _decode(descriptor: str, value: Any) -> Any:",
        "    if descriptor.startswith('finite:'):",
        "        cls = globals().get(descriptor.split(':', 1)[1])",
        "        if cls is None:",
        "            raise BindingError(f'generated finite type is missing: {descriptor}')",
        "        try:",
        "            return cls(_variant(value))",
        "        except ValueError as error:",
        "            raise BindingError(f'unknown variant for {descriptor}: {value!r}') from error",
        "    if descriptor.startswith('record:'):",
        "        cls = globals().get(descriptor.split(':', 1)[1])",
        "        if cls is None:",
        "            raise BindingError(f'generated record type is missing: {descriptor}')",
        "        return cls.from_host_value(value)",
        "    if descriptor.startswith('sequence:') or descriptor.startswith('vector:'):",
        "        sequence = value.get('sequence') if isinstance(value, dict) else None",
        "        values = sequence.get('values') if isinstance(sequence, dict) else None",
        "        if not isinstance(values, list):",
        "            raise BindingError('returned value is not a typed sequence')",
        "        parts = descriptor.split(':')",
        "        element_descriptor = ':'.join(parts[1:-1])",
        "        return tuple(_decode(element_descriptor, item) for item in values)",
        "    if descriptor == 'bool':",
        "        boolean = value.get('boolean') if isinstance(value, dict) else None",
        "        return boolean.get('value') if isinstance(boolean, dict) else value",
        "    if descriptor == 'int':",
        "        integer = value.get('integer') if isinstance(value, dict) else None",
        "        return integer.get('value') if isinstance(integer, dict) else value",
        "    if descriptor == 'float':",
        "        floating = value.get('float') if isinstance(value, dict) else None",
        "        return floating.get('value') if isinstance(floating, dict) else value",
        "    if isinstance(value, dict):",
        "        for key in ('boolean', 'integer', 'float', 'byte'):",
        "            payload = value.get(key)",
        "            if isinstance(payload, dict) and 'value' in payload:",
        "                return payload['value']",
        "    return value",
        "",
    ]

    for name, finite in finite_types.items():
        out.append(f"class {safe_identifier(name)}(str, Enum):")
        variants = finite.get("variant_names", {})
        for _, variant in sorted(variants.items(), key=lambda item: int(item[0])):
            out.append(f"    {safe_identifier(variant)} = {variant!r}")
        out.extend(["", ""])

    for name, record in record_types.items():
        py_name = safe_identifier(name)
        fields = record.get("fields", [])
        out.append("@dataclass(frozen=True)")
        out.append(f"class {py_name}:")
        descriptors: list[tuple[str, str]] = []
        for field in fields:
            if not isinstance(field, list) or len(field) != 2:
                continue
            field_name = safe_identifier(str(field[0]))
            descriptor = python_descriptor(field[1])
            descriptors.append((field_name, descriptor))
            out.append(f"    {field_name}: {python_annotation(descriptor)}")
        if not descriptors:
            out.append("    pass")
        out.append("")
        out.append("    def to_host_value(self) -> dict[str, Any]:")
        out.append("        return {'record': {'type': self.__class__.__name__, 'fields': {")
        for field_name, _ in descriptors:
            out.append(f"            {field_name!r}: _encode(self.{field_name}),")
        out.append("        }}}")
        out.append("")
        out.append("    @classmethod")
        out.append(f"    def from_host_value(cls, value: Any) -> {py_name}:")
        out.append("        fields = _fields(value)")
        for field_name, descriptor in descriptors:
            out.append(f"        {field_name} = _decode({descriptor!r}, fields.get({field_name!r}))")
        args = ", ".join(f"{field}={field}" for field, _ in descriptors)
        out.append(f"        return cls({args})")
        out.extend(["", ""])

    out.extend([
        "@dataclass(frozen=True)",
        "class _BindingConfig:",
        "    mncs: str",
        "    source: Path",
        "    libraries: tuple[Path, ...] = ()",
        "    timeout: float = 60.0",
        "",
        "class Binding:",
        "    def __init__(self, mncs: str, source: str | Path, *, libraries: tuple[str | Path, ...] = (), timeout: float = 60.0) -> None:",
        "        self._config = _BindingConfig(mncs, Path(source), tuple(Path(path) for path in libraries), timeout)",
        "        self.last_execution: dict[str, Any] | None = None",
        "",
        "    def _call(self, module: str, function: str, argument: Any) -> dict[str, Any]:",
        "        request = {'schema_version': '0.1', 'target': {'module': module, 'function': function},",
        "                   'typed_arguments': [_encode(argument)], 'expected_interface_identity': INTERFACE_IDENTITY, 'step_budget': 8192}",
        "        with tempfile.TemporaryDirectory(prefix='mncs-generated-binding-') as directory:",
        "            request_path = Path(directory) / 'request.json'",
        "            request_path.write_text(json.dumps(request, separators=(',', ':')), encoding='utf-8')",
        "            environment = dict(os.environ)",
        "            if self._config.libraries:",
        "                environment['MNCS_LIBRARY_PATH'] = os.pathsep.join(str(path.resolve()) for path in self._config.libraries)",
        "            result = subprocess.run([self._config.mncs, 'execute', str(self._config.source), str(request_path)],",
        "                                     text=True, capture_output=True, env=environment, timeout=self._config.timeout)",
        "        if result.returncode != 0:",
        "            detail = result.stderr.strip() or result.stdout.strip() or 'typed call failed'",
        "            raise BindingError(detail)",
        "        try:",
        "            response = json.loads(result.stdout)",
        "        except json.JSONDecodeError as error:",
        "            raise BindingError('mncs returned non-JSON output') from error",
        "        if response.get('status') != 'returned':",
        "            raise BindingError(f'mncs typed call did not return: {response!r}')",
        "        self.last_execution = response",
        "        return response",
        "",
    ])

    functions = abi["functions"]
    for function_name, function in sorted(functions.items()):
        inputs = function.get("inputs", [])
        input_names = function.get("input_names", [])
        output_types = function.get("outputs", [])
        if len(inputs) != 1 or len(output_types) != 1:
            continue
        arg_descriptor = python_descriptor(inputs[0])
        return_descriptor = python_descriptor(output_types[0])
        arg_annotation = python_annotation(arg_descriptor)
        return_annotation = python_annotation(return_descriptor)
        method = safe_identifier(function_name)
        out.extend([
            f"    def {method}(self, input_value: {arg_annotation}) -> {return_annotation}:",
            f"        response = self._call({function.get('declaring_module', abi['module'])!r}, {function_name!r}, input_value)",
            f"        return _decode({return_descriptor!r}, response['returned'][0])",
            "",
        ])

    out.extend([
        "",
        "BINDING_METADATA = {",
        "    'generator_version': GENERATOR_VERSION,",
        "    'module_identity': MODULE_IDENTITY,",
        "    'interface_identity': INTERFACE_IDENTITY,",
        "    'typed_call_schema_version': TYPED_CALL_SCHEMA_VERSION,",
        "    'binding_content_identity': BINDING_CONTENT_IDENTITY,",
        "}",
        "",
    ])
    return "\n".join(out)


def rust_type(contract: Any) -> str:
    descriptor = python_descriptor(contract)
    return rust_type_descriptor(descriptor)


def rust_type_descriptor(descriptor: str) -> str:
    if descriptor == "bool":
        return "bool"
    if descriptor == "int":
        return "i64"
    if descriptor == "float":
        return "f64"
    if descriptor.startswith("finite:") or descriptor.startswith("record:"):
        return safe_identifier(descriptor.split(":", 1)[1])
    if descriptor.startswith(("sequence:", "vector:")):
        parts = descriptor.split(":")
        element = ":".join(parts[1:-1])
        return f"Vec<{rust_type_descriptor(element)}>"
    if descriptor.startswith("mask:"):
        return "Vec<bool>"
    return "serde_json::Value"


def rust_encode_expression(descriptor: str, expression: str) -> str:
    if descriptor == "bool":
        return f"json!({{\"boolean\": {{\"value\": {expression}}}}})"
    if descriptor == "int":
        return f"json!({{\"integer\": {{\"value\": {expression}}}}})"
    if descriptor == "float":
        return f"json!({{\"float\": {{\"value\": {expression}}}}})"
    if descriptor.startswith(("finite:", "record:")):
        return f"{expression}.host_value()"
    if descriptor.startswith(("sequence:", "vector:")):
        parts = descriptor.split(":")
        element = ":".join(parts[1:-1])
        item = rust_encode_expression(element, "item")
        return f"json!({{\"sequence\": {{\"values\": {expression}.iter().map(|item| {item}).collect::<Vec<_>>()}}}})"
    if descriptor.startswith("mask:"):
        return f"json!({{\"mask\": {{\"lanes\": {expression}}}}})"
    return expression


def rust_decode_result_expression(descriptor: str, expression: str) -> str:
    if descriptor == "bool":
        return f"decode_bool({expression})"
    if descriptor == "int":
        return f"decode_i64({expression})"
    if descriptor == "float":
        return f"decode_f64({expression})"
    if descriptor.startswith("finite:") or descriptor.startswith("record:"):
        return f"{safe_identifier(descriptor.split(':', 1)[1])}::from_host_value({expression})"
    if descriptor.startswith(("sequence:", "vector:")):
        parts = descriptor.split(":")
        element = ":".join(parts[1:-1])
        item_result = rust_decode_result_expression(element, "item")
        return (
            f"sequence_values({expression})?.iter().map(|item| {item_result})"
            " .collect::<Result<Vec<_>, _>>()"
        )
    if descriptor.startswith("mask:"):
        return f"mask_values({expression})"
    return f"Ok({expression})"


def rust_decode_expression(descriptor: str, expression: str) -> str:
    return f"{rust_decode_result_expression(descriptor, expression)}?"


def rust_binding(abi: dict[str, Any]) -> str:
    composites = unique_composites(abi)
    finite_types = {
        name: contract["finite"]
        for name, contract in composites.items()
        if "finite" in contract
    }
    record_types = {
        name: contract["record"]
        for name, contract in composites.items()
        if "record" in contract
    }
    out = [
        "// Generated by mncs-language/scripts/generate_host_bindings.py. Do not edit.",
        "use mncs_embed::{CallOptions, EmbedError, Session};",
        "use serde_json::{json, Map, Value};",
        "",
        f"pub const GENERATOR_VERSION: &str = {json.dumps(GENERATOR_VERSION)};",
        f"pub const MODULE_IDENTITY: &str = {json.dumps(abi['module'])};",
        f"pub const INTERFACE_IDENTITY: &str = {json.dumps(abi['interface_identity'])};",
        f"pub const TYPED_CALL_SCHEMA_VERSION: &str = {json.dumps(abi.get('typed_call_schema_version', 'mncs.typed-call/1'))};",
        f"pub const BINDING_CONTENT_IDENTITY: &str = {json.dumps(binding_content_identity(abi, 'rust'))};",
        "",
        "trait HostValue { fn host_value(&self) -> Value; }",
        "",
        "fn decode_object<'a>(value: &'a Value, kind: &str) -> Result<&'a Map<String, Value>, EmbedError> {",
        "    value.as_object().ok_or_else(|| EmbedError::new(\"binding_decode\", format!(\"typed {kind} is not an object\")))",
        "}",
        "fn decode_bool(value: &Value) -> Result<bool, EmbedError> {",
        "    decode_object(value, \"boolean\")?.get(\"boolean\").and_then(Value::as_object).and_then(|v| v.get(\"value\")).and_then(Value::as_bool).ok_or_else(|| EmbedError::new(\"binding_decode\", \"typed boolean is malformed\"))",
        "}",
        "fn decode_i64(value: &Value) -> Result<i64, EmbedError> {",
        "    decode_object(value, \"integer\")?.get(\"integer\").and_then(Value::as_object).and_then(|v| v.get(\"value\")).and_then(Value::as_i64).ok_or_else(|| EmbedError::new(\"binding_decode\", \"typed integer is malformed\"))",
        "}",
        "fn decode_f64(value: &Value) -> Result<f64, EmbedError> {",
        "    decode_object(value, \"float\")?.get(\"float\").and_then(Value::as_object).and_then(|v| v.get(\"value\")).and_then(Value::as_f64).ok_or_else(|| EmbedError::new(\"binding_decode\", \"typed float is malformed\"))",
        "}",
        "fn sequence_values(value: &Value) -> Result<&Vec<Value>, EmbedError> {",
        "    decode_object(value, \"sequence\")?.get(\"sequence\").and_then(Value::as_object).and_then(|v| v.get(\"values\")).and_then(Value::as_array).ok_or_else(|| EmbedError::new(\"binding_decode\", \"typed sequence is malformed\"))",
        "}",
        "fn mask_values(value: &Value) -> Result<Vec<bool>, EmbedError> {",
        "    let lanes = decode_object(value, \"mask\")?.get(\"mask\").and_then(Value::as_object).and_then(|v| v.get(\"lanes\")).and_then(Value::as_array).ok_or_else(|| EmbedError::new(\"binding_decode\", \"typed mask is malformed\"))?;",
        "    lanes.iter().map(|item| item.as_bool().ok_or_else(|| EmbedError::new(\"binding_decode\", \"typed mask lane is malformed\"))).collect()",
        "}",
        "fn finite_variant(value: &Value) -> Result<&str, EmbedError> {",
        "    let finite = decode_object(value, \"finite\")?.get(\"finite\").and_then(Value::as_object).ok_or_else(|| EmbedError::new(\"binding_decode\", \"typed finite is malformed\"))?;",
        "    if let Some(identity) = finite.get(\"variant_identity\").and_then(Value::as_str) {",
        "        return Ok(identity.rsplit(\"::\").next().unwrap_or(identity));",
        "    }",
        "    finite.get(\"variant\").and_then(Value::as_str).ok_or_else(|| EmbedError::new(\"binding_decode\", \"typed finite has no variant identity\"))",
        "}",
        "fn record_fields<'a>(value: &'a Value) -> Result<&'a Map<String, Value>, EmbedError> {",
        "    let record = decode_object(value, \"record\")?.get(\"record\").and_then(Value::as_object).ok_or_else(|| EmbedError::new(\"binding_decode\", \"typed record is malformed\"))?;",
        "    record.get(\"fields\").and_then(Value::as_object).ok_or_else(|| EmbedError::new(\"binding_decode\", \"typed record fields are malformed\"))",
        "}",
        "fn returned_value(output: &mncs_embed::CallOutput) -> Result<Value, EmbedError> {",
        "    if output.status != \"returned\" { return Err(EmbedError::new(\"binding_call\", format!(\"typed call returned {}\", output.status))); }",
        "    let value = output.returned.first().ok_or_else(|| EmbedError::new(\"binding_decode\", \"typed call returned no value\"))?;",
        "    serde_json::to_value(value).map_err(|error| EmbedError::new(\"binding_decode\", error.to_string()))",
        "}",
        "",
    ]

    for name, finite in sorted(finite_types.items()):
        py_name = safe_identifier(name)
        out.extend(["#[derive(Debug, Clone, Copy, PartialEq, Eq)]", f"pub enum {py_name} {{"])
        variants = finite.get("variant_names", {})
        for _, variant in sorted(variants.items(), key=lambda item: int(item[0])):
            out.append(f"    {safe_identifier(variant)},")
        out.extend(["}", "", f"impl HostValue for {py_name} {{", "    fn host_value(&self) -> Value {", "        let variant = match self {"])
        for _, variant in sorted(variants.items(), key=lambda item: int(item[0])):
            out.append(f"            Self::{safe_identifier(variant)} => {json.dumps(variant)},")
        out.extend(["        };", f"        json!({{\"finite\": {{\"type\": {json.dumps(py_name)}, \"variant\": variant}}}})", "    }", "}", ""])
        out.extend([f"impl {py_name} {{", "    fn from_host_value(value: &Value) -> Result<Self, EmbedError> {", "        match finite_variant(value)? {"])
        for _, variant in sorted(variants.items(), key=lambda item: int(item[0])):
            out.append(f"            {json.dumps(variant)} => Ok(Self::{safe_identifier(variant)}),")
        out.extend([f"            other => Err(EmbedError::new(\"binding_decode\", format!(\"unknown {py_name} variant {{other}}\"))),", "        }", "    }", "}", ""])

    for name, record in sorted(record_types.items()):
        py_name = safe_identifier(name)
        fields = [field for field in record.get("fields", []) if isinstance(field, list) and len(field) == 2]
        out.extend(["#[derive(Debug, Clone, PartialEq)]", f"pub struct {py_name} {{"])
        descriptors: list[tuple[str, str]] = []
        for field_name, contract in fields:
            field = safe_identifier(str(field_name))
            descriptor = python_descriptor(contract)
            descriptors.append((field, descriptor))
            out.append(f"    pub {field}: {rust_type(contract)},")
        out.extend(["}", "", f"impl HostValue for {py_name} {{", "    fn host_value(&self) -> Value {", "        json!({\"record\": {\"type\": " + json.dumps(py_name) + ", \"fields\": {"])
        for field, descriptor in descriptors:
            out.append(f"            {json.dumps(field)}: {rust_encode_expression(descriptor, f'self.{field}')},")
        out.extend(["        }}})", "    }", "}", ""])
        out.extend([f"impl {py_name} {{", "    fn from_host_value(value: &Value) -> Result<Self, EmbedError> {", "        let fields = record_fields(value)?;"])
        for field, descriptor in descriptors:
            out.append(f"        let {field} = {rust_decode_expression(descriptor, f'fields.get({json.dumps(field)}).ok_or_else(|| EmbedError::new(\"binding_decode\", \"missing record field\"))?')};")
        out.append("        Ok(Self {")
        for field, _ in descriptors:
            out.append(f"            {field},")
        out.extend(["        })", "    }", "}", ""])

    functions = abi["functions"]
    out.extend([
        "pub fn typed_call(",
        "    session: &Session,",
        "    module: &str,",
        "    function: &str,",
        "    arguments: &str,",
        "    mut options: CallOptions,",
        ") -> Result<mncs_embed::CallOutput, EmbedError> {",
        "    options.expected_interface_identity = Some(INTERFACE_IDENTITY.to_owned());",
        "    session.call_typed_json(module, function, arguments, &options)",
        "}",
        "",
    ])
    for function_name, function in sorted(functions.items()):
        inputs = function.get("inputs", [])
        outputs = function.get("outputs", [])
        if len(inputs) != 1 or len(outputs) != 1:
            continue
        arg_descriptor = python_descriptor(inputs[0])
        return_descriptor = python_descriptor(outputs[0])
        method = safe_identifier(function_name)
        arg_type = rust_type(inputs[0])
        return_type = rust_type(outputs[0])
        encoded = rust_encode_expression(arg_descriptor, "input")
        decoded = rust_decode_expression(return_descriptor, "&value")
        out.extend([
            f"pub fn {method}(session: &Session, input: {arg_type}, options: CallOptions) -> Result<{return_type}, EmbedError> {{",
            f"    let arguments = serde_json::to_string(&vec![{encoded}]).map_err(|error| EmbedError::new(\"binding_encode\", error.to_string()))?;",
            f"    let output = typed_call(session, {json.dumps(function.get('declaring_module', abi['module']))}, {json.dumps(function_name)}, &arguments, options)?;",
            "    let value = returned_value(&output)?;",
            f"    Ok({decoded})",
            "}",
            "",
        ])
    out.extend([
        "pub const BINDING_METADATA: (&str, &str, &str, &str, &str) = (",
        "    GENERATOR_VERSION, MODULE_IDENTITY, INTERFACE_IDENTITY,",
        "    TYPED_CALL_SCHEMA_VERSION, BINDING_CONTENT_IDENTITY,",
        ");",
        "",
    ])
    return "\n".join(out)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--abi", type=Path, required=True)
    parser.add_argument("--language", choices=("python", "rust"), required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    abi = load_abi(args.abi)
    rendered = python_binding(abi) if args.language == "python" else rust_binding(abi)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(rendered + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
