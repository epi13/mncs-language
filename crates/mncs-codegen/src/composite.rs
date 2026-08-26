//! The MNCS canonical composite cell model, shared by every executable
//! backend realization.
//!
//! Composite values (records and payload-bearing finite variants) are
//! immutable cells in a backend-provided byte arena. The layout below is the
//! language-owned contract; WASM linear memory, C11 static arena, LLVM
//! globals, and Cranelift cell libcalls all realize the same byte layout so
//! an arena image is interchangeable between realizations.
//!
//! Canonical cell layout v0.1:
//! - every cell is 8-byte aligned and every field occupies one 8-byte slot;
//! - record cells place field `i` at byte offset `i * 8` in canonical
//!   (name-sorted) field order;
//! - boxed finite cells store the variant discriminant as an i32 in slot 0
//!   and payload field `i` at byte offset `(i + 1) * 8`;
//! - a field whose declared type is itself composite stores that value's
//!   arena offset (a cell reference), not an inline copy;
//! - integer fields of 64 bits occupy the full slot; narrower integers,
//!   booleans, finite discriminants, and cell references occupy the low 4
//!   bytes of their slot.

use std::collections::BTreeMap;

use mncs_model::{Program, SemanticId};

/// Byte width of one stored field inside its 8-byte slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotWidth {
    W32,
    W64,
}

/// One canonical field entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellField {
    pub name: String,
    pub width: SlotWidth,
}

/// Derived canonical layout for one program's composite types.
#[derive(Debug, Clone, Default)]
pub struct CompositeLayout {
    /// record identity -> ordered canonical fields
    pub records: BTreeMap<SemanticId, Vec<CellField>>,
    /// boxed finite identity -> discriminant -> payload fields
    pub boxed_finites: BTreeMap<SemanticId, BTreeMap<u32, Vec<CellField>>>,
}

impl CompositeLayout {
    /// Derive the canonical layout from the language-owned declarations.
    /// Field order is the declaration's canonical (name-sorted) order; slot
    /// widths follow declared semantic types exactly as on portable WASM.
    pub fn from_program(program: &Program) -> Self {
        let field_of = |name: &str, semantic_type: &str| CellField {
            name: name.to_owned(),
            width: slot_width(semantic_type, program),
        };
        let mut layout = Self::default();
        for record in &program.record_types {
            layout.records.insert(
                record.identity.clone(),
                record
                    .fields
                    .iter()
                    .map(|field| field_of(&field.name, &field.field_type))
                    .collect(),
            );
        }
        for finite in &program.finite_types {
            let boxed = finite
                .variants
                .iter()
                .any(|variant| !variant.payload.is_empty());
            if !boxed {
                continue;
            }
            let variants = finite
                .variants
                .iter()
                .map(|variant| {
                    (
                        variant.discriminant,
                        variant
                            .payload
                            .iter()
                            .map(|field| field_of(&field.name, &field.field_type))
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            layout
                .boxed_finites
                .insert(finite.identity.clone(), variants);
        }
        layout
    }

    pub fn is_boxed_finite(&self, type_identity: &SemanticId) -> bool {
        self.boxed_finites.contains_key(type_identity)
    }
}

/// A finite type is boxed exactly when some variant declares payload
/// fields; per-variant entries exist even for payload-free variants.
pub fn finite_payloads_declare_payloads(payloads: &BTreeMap<u32, Vec<(String, String)>>) -> bool {
    payloads.values().any(|fields| !fields.is_empty())
}

fn slot_width(semantic_type: &str, program: &mncs_model::Program) -> SlotWidth {
    // Records always occupy referenced cells. Payload-bearing finite
    // variants are boxed cells too; payload-free variants are bare tags and
    // stay in a 32-bit slot. Sequences ride full words (cell refs or packed
    // view descriptors).
    if program
        .record_types
        .iter()
        .any(|record| record.name == semantic_type)
    {
        return SlotWidth::W64;
    }
    if let Some(finite) = program
        .finite_types
        .iter()
        .find(|finite| finite.name == semantic_type)
    {
        let boxed = finite
            .variants
            .iter()
            .any(|variant| !variant.payload.is_empty());
        return if boxed {
            SlotWidth::W64
        } else {
            SlotWidth::W32
        };
    }
    match mncs_model::BodyType::from_semantic_name(semantic_type) {
        mncs_model::BodyType::Integer(ty) if ty.bits == 64 => SlotWidth::W64,
        _ => SlotWidth::W32,
    }
}

fn slot_width_registry(
    semantic_type: &str,
    registry: &BTreeMap<String, mncs_model::BackendValueContract>,
) -> SlotWidth {
    if registry.values().any(|contract| match contract {
        mncs_model::BackendValueContract::Record { name, .. } => name == semantic_type,
        _ => false,
    }) {
        return SlotWidth::W64;
    }
    match mncs_model::BodyType::from_semantic_name(semantic_type) {
        mncs_model::BodyType::Integer(ty) if ty.bits == 64 => SlotWidth::W64,
        _ => SlotWidth::W32,
    }
}

/// What crosses the function boundary for one argument or result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryValue {
    /// Raw 64-bit pattern passed as the scalar parameter.
    Bits(u64),
    /// Arena byte offset of a canonical cell.
    Cell(u64),
}

/// Host-side canonical cell writer: marshals composite arguments into an
/// arena image before a native-process call. Byte layout mirrors the cell
/// contract above exactly.
pub struct ArenaWriter {
    image: Vec<u8>,
    registry: BTreeMap<String, mncs_model::BackendValueContract>,
}

impl ArenaWriter {
    pub fn new(registry: BTreeMap<String, mncs_model::BackendValueContract>) -> Self {
        Self {
            image: Vec::new(),
            registry,
        }
    }

    pub fn into_image(self) -> Vec<u8> {
        self.image
    }

    fn align8(&mut self) -> u64 {
        let misalign = self.image.len() as u64 % 8;
        if misalign != 0 {
            self.image
                .resize(self.image.len() + (8 - misalign) as usize, 0);
        }
        self.image.len() as u64
    }

    fn put32(&mut self, offset: u64, value: u32) {
        self.image[offset as usize..offset as usize + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn put64(&mut self, offset: u64, value: u64) {
        self.image[offset as usize..offset as usize + 8].copy_from_slice(&value.to_le_bytes());
    }

    /// Encode one top-level argument. Scalars and unboxed finite variants
    /// cross as raw bits; composites allocate cells.
    pub fn encode_argument(
        &mut self,
        value: &mncs_model::ExecutionValue,
    ) -> Result<BoundaryValue, String> {
        match value {
            mncs_model::ExecutionValue::Integer { value, .. } => {
                Ok(BoundaryValue::Bits(*value as u64))
            }
            mncs_model::ExecutionValue::Boolean { value } => {
                Ok(BoundaryValue::Bits(u64::from(*value)))
            }
            mncs_model::ExecutionValue::Byte { value } => Ok(BoundaryValue::Bits(*value as u64)),
            mncs_model::ExecutionValue::Finite {
                payload,
                discriminant,
                type_identity,
                ..
            } if !self.identity_is_boxed(type_identity) && payload.is_empty() => {
                Ok(BoundaryValue::Bits(u64::from(*discriminant)))
            }
            other => Ok(BoundaryValue::Cell(self.encode_cell(other)?)),
        }
    }

    fn identity_is_boxed(&self, type_identity: &mncs_model::SemanticId) -> bool {
        self.registry.values().any(|contract| {
            matches!(
                contract,
                mncs_model::BackendValueContract::Finite {
                    type_identity: identity,
                    payloads,
                    ..
                } if identity == type_identity && finite_payloads_declare_payloads(payloads)
            )
        })
    }

    /// Encode any composite value into a fresh canonical cell; returns its
    /// byte offset.
    fn encode_cell(&mut self, value: &mncs_model::ExecutionValue) -> Result<u64, String> {
        use mncs_model::{BackendValueContract as Contract, ExecutionValue as Value};
        let base = self.align8();
        match value {
            Value::Record { fields, .. } => {
                let Some(Contract::Record {
                    fields: declared, ..
                }) = value_record_contract(value, &self.registry)
                else {
                    return Err("record value has no declared canonical layout".to_owned());
                };
                let declared = declared.clone();
                if declared.len() != fields.len() {
                    return Err("record field count does not match the declared layout".to_owned());
                }
                self.image.resize(base as usize + declared.len() * 8, 0);
                for (index, ((name, declared_type), (field_name, field_value))) in
                    declared.iter().zip(fields.iter()).enumerate()
                {
                    if name != field_name {
                        return Err(format!(
                            "record field {field_name:?} does not match the canonical layout {name:?}"
                        ));
                    }
                    self.store_field(base + index as u64 * 8, field_value, declared_type)?;
                }
                Ok(base)
            }
            Value::Sequence { values } => {
                let elements = values.clone();
                self.encode_sequence(&elements)
            }
            Value::Finite {
                discriminant,
                payload,
                ..
            } => {
                let Some(variant_fields) = value_finite_payloads(value, &self.registry) else {
                    return Err("finite value has no boxed layout in this program".to_owned());
                };
                let variant_fields = variant_fields.clone();
                if variant_fields.len() != payload.len() {
                    return Err("payload field count does not match the declared layout".to_owned());
                }
                self.image
                    .resize(base as usize + (variant_fields.len() + 1) * 8, 0);
                self.put32(base, *discriminant);
                for (index, ((name, declared_type), (field_name, field_value))) in
                    variant_fields.iter().zip(payload.iter()).enumerate()
                {
                    if name != field_name {
                        return Err(format!(
                            "payload field {field_name:?} does not match the canonical layout {name:?}"
                        ));
                    }
                    self.store_field(base + (index as u64 + 1) * 8, field_value, declared_type)?;
                }
                Ok(base)
            }
            other => Err(format!("value cannot occupy a canonical cell: {other:?}")),
        }
    }

    /// Store one field at an exact byte offset; nested composites allocate
    /// their own cells first and store the resulting reference.
    fn store_field(
        &mut self,
        offset: u64,
        value: &mncs_model::ExecutionValue,
        declared_type: &str,
    ) -> Result<(), String> {
        use mncs_model::ExecutionValue as Value;
        // Composite-typed fields always cross as cell references.
        let width = match value {
            Value::Record { .. } | Value::Sequence { .. } => SlotWidth::W64,
            _ => slot_width_registry(declared_type, &self.registry),
        };
        match value {
            Value::Record { .. } => {
                // Records are always composite references.
                let cell = self.encode_cell(value)?;
                self.put64(offset, cell);
                Ok(())
            }
            Value::Finite {
                type_identity,
                discriminant,
                payload,
                ..
            } => {
                // Boxed variants occupy cells; unboxed ones are bare tags.
                if !payload.is_empty() || self.identity_is_boxed(type_identity) {
                    let cell = self.encode_cell(value)?;
                    self.put64(offset, cell);
                    Ok(())
                } else {
                    self.put32(offset, *discriminant);
                    Ok(())
                }
            }
            Value::Integer { value, .. } => {
                match width {
                    SlotWidth::W32 => self.put32(offset, *value as i32 as u32),
                    SlotWidth::W64 => self.put64(offset, *value as u64),
                }
                Ok(())
            }
            Value::Boolean { value } => {
                self.put32(offset, u32::from(*value));
                Ok(())
            }
            Value::Byte { value } => {
                self.put32(offset, *value as u32);
                Ok(())
            }
            // Exact-length sequences occupy their own cells and are
            // referenced from this slot.
            seq @ Value::Sequence { .. } => {
                let cell = self.encode_cell(seq)?;
                self.put64(offset, cell);
                Ok(())
            }
            Value::Vector { .. } | Value::Mask { .. } => Err(
                "vector and mask values do not cross the canonical composite boundary".to_owned(),
            ),
        }
    }

    /// Encode an exact-length sequence into a fresh canonical cell: one
    /// 8-byte slot per element in index order. Returns the cell offset.
    fn encode_sequence(&mut self, elements: &[mncs_model::ExecutionValue]) -> Result<u64, String> {
        let base = self.align8();
        self.image.resize(base as usize + elements.len() * 8, 0);
        for (index, element) in elements.iter().enumerate() {
            let slot = base + index as u64 * 8;
            let bits = element_slot_bits(element)?;
            self.put64(slot, bits);
        }
        Ok(base)
    }
}

/// Zero-extended slot bits for a scalar or byte sequence element; nested
/// composites are rejected because they must occupy referenced cells.
fn element_slot_bits(value: &mncs_model::ExecutionValue) -> Result<u64, String> {
    match value {
        mncs_model::ExecutionValue::Integer { value, .. } => Ok(*value as u64),
        mncs_model::ExecutionValue::Boolean { value } => Ok(u64::from(*value)),
        mncs_model::ExecutionValue::Byte { value } => Ok(*value as u64),
        mncs_model::ExecutionValue::Finite { discriminant, .. } => Ok(u64::from(*discriminant)),
        other => Err(format!(
            "nested composite sequence element requires cell realization: {other:?}"
        )),
    }
}

fn value_record_contract<'a>(
    value: &mncs_model::ExecutionValue,
    registry: &'a BTreeMap<String, mncs_model::BackendValueContract>,
) -> Option<&'a mncs_model::BackendValueContract> {
    let mncs_model::ExecutionValue::Record { type_identity, .. } = value else {
        return None;
    };
    registry.values().find(|contract| {
        matches!(contract, mncs_model::BackendValueContract::Record {
            type_identity: identity,
            ..
        } if identity == type_identity)
    })
}

fn value_finite_payloads<'a>(
    value: &mncs_model::ExecutionValue,
    registry: &'a BTreeMap<String, mncs_model::BackendValueContract>,
) -> Option<&'a Vec<(String, String)>> {
    let mncs_model::ExecutionValue::Finite {
        type_identity,
        discriminant,
        ..
    } = value
    else {
        return None;
    };
    registry.values().find_map(|contract| match contract {
        mncs_model::BackendValueContract::Finite {
            type_identity: identity,
            payloads,
            ..
        } if identity == type_identity => payloads.get(discriminant),
        _ => None,
    })
}

/// Host-side reader over a finished arena image.
pub struct ArenaReader<'a> {
    image: &'a [u8],
    registry: &'a BTreeMap<String, mncs_model::BackendValueContract>,
}

impl<'a> ArenaReader<'a> {
    pub fn new(
        image: &'a [u8],
        registry: &'a BTreeMap<String, mncs_model::BackendValueContract>,
    ) -> Self {
        Self { image, registry }
    }

    fn get32(&self, offset: u64) -> Result<u32, String> {
        let bytes = self
            .image
            .get(offset as usize..offset as usize + 4)
            .ok_or_else(|| "cell read exceeds arena image".to_owned())?;
        Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn get64(&self, offset: u64) -> Result<u64, String> {
        let bytes = self
            .image
            .get(offset as usize..offset as usize + 8)
            .ok_or_else(|| "cell read exceeds arena image".to_owned())?
            .try_into()
            .unwrap();
        Ok(u64::from_le_bytes(bytes))
    }

    /// Decode one value at `root` against a named declared semantic type.
    pub fn decode(
        &self,
        root: u64,
        contract: &mncs_model::BackendValueContract,
    ) -> Result<mncs_model::ExecutionValue, String> {
        use mncs_model::{BackendValueContract as Contract, ExecutionValue as Value};
        match contract {
            Contract::Scalar { .. } => {
                Err("scalar results decode outside the cell codec".to_owned())
            }
            Contract::Record {
                type_identity,
                name,
                fields,
            } => {
                let mut decoded = Vec::new();
                for (index, (field_name, declared_type)) in fields.iter().enumerate() {
                    decoded.push((
                        field_name.clone(),
                        self.decode_field(root + index as u64 * 8, declared_type)?,
                    ));
                }
                Ok(Value::Record {
                    type_identity: type_identity.clone(),
                    name: name.clone(),
                    fields: decoded,
                })
            }
            Contract::Finite {
                type_identity,
                variants,
                payloads,
            } => {
                let discriminant = self.get32(root)?;
                let Some(variant_identity) = variants.get(&discriminant) else {
                    return Err(format!("decoded discriminant {discriminant} is undeclared"));
                };
                let empty = Vec::new();
                let fields = payloads.get(&discriminant).unwrap_or(&empty);
                let mut payload = Vec::new();
                for (index, (field_name, declared_type)) in fields.iter().enumerate() {
                    payload.push((
                        field_name.clone(),
                        self.decode_field(root + (index as u64 + 1) * 8, declared_type)?,
                    ));
                }
                Ok(Value::Finite {
                    type_identity: type_identity.clone(),
                    variant_identity: variant_identity.clone(),
                    discriminant,
                    payload,
                })
            }
        }
    }

    fn resolve_contract(
        declared_type: &str,
        registry: &'a BTreeMap<String, mncs_model::BackendValueContract>,
    ) -> Option<&'a mncs_model::BackendValueContract> {
        match mncs_model::BodyType::from_semantic_name(declared_type) {
            mncs_model::BodyType::Integer(_) => None,
            mncs_model::BodyType::Named(name) if name == "bool" => None,
            _ => registry.get(declared_type),
        }
    }

    fn decode_field(
        &self,
        offset: u64,
        declared_type: &str,
    ) -> Result<mncs_model::ExecutionValue, String> {
        use mncs_model::{BackendValueContract as Contract, BodyType};
        // Boxed finite fields hold cell references; unboxed finite fields
        // hold bare discriminants, exactly as the writer stores them.
        if let Some(Contract::Finite { payloads, .. }) =
            Self::resolve_contract(declared_type, self.registry)
        {
            if !payloads.is_empty() {
                let reference = self.get64(offset)?;
                let contract =
                    Self::resolve_contract(declared_type, self.registry).expect("resolved above");
                return self.decode(reference, contract);
            }
            let discriminant = self.get32(offset)?;
            let Some(Contract::Finite {
                type_identity,
                variants,
                ..
            }) = Self::resolve_contract(declared_type, self.registry)
            else {
                unreachable!("resolved above");
            };
            let Some(variant_identity) = variants.get(&discriminant) else {
                return Err(format!("decoded discriminant {discriminant} is undeclared"));
            };
            return Ok(mncs_model::ExecutionValue::Finite {
                type_identity: type_identity.clone(),
                variant_identity: variant_identity.clone(),
                discriminant,
                payload: Vec::new(),
            });
        }
        if let Some(contract @ Contract::Record { .. }) =
            Self::resolve_contract(declared_type, self.registry)
        {
            let reference = self.get64(offset)?;
            return self.decode(reference, contract);
        }
        match BodyType::from_semantic_name(declared_type) {
            BodyType::Integer(ty) => {
                let value: i128 = match slot_width_registry(declared_type, self.registry) {
                    SlotWidth::W32 => {
                        let bits = self.get32(offset)?;
                        if ty.signed {
                            i32::from_le_bytes(bits.to_le_bytes()) as i128
                        } else {
                            bits as i128
                        }
                    }
                    SlotWidth::W64 => self.get64(offset)? as i64 as i128,
                };
                Ok(mncs_model::ExecutionValue::Integer { value, ty })
            }
            BodyType::Named(name) if name == "bool" => Ok(mncs_model::ExecutionValue::Boolean {
                value: self.get32(offset)? == 1,
            }),
            _ => Err(format!(
                "field type {declared_type} has no contract for decoding"
            )),
        }
    }
}

#[cfg(test)]
mod codec_tests {
    use super::*;
    use mncs_model::{BackendValueContract, ExecutionValue, SemanticId};

    fn sid(s: &str) -> SemanticId {
        SemanticId(s.to_owned())
    }

    #[test]
    fn status_pair_record_encodes_to_the_canonical_layout() {
        let status = BackendValueContract::Finite {
            type_identity: sid("T:Status"),
            variants: BTreeMap::from([
                (0, sid("V:PASS")),
                (1, sid("V:FAIL")),
                (2, sid("V:UNKNOWN")),
            ]),
            payloads: BTreeMap::new(),
        };
        let pair = BackendValueContract::Record {
            type_identity: sid("T:StatusPair"),
            name: "StatusPair".to_owned(),
            fields: vec![
                ("left".to_owned(), "Status".to_owned()),
                ("right".to_owned(), "Status".to_owned()),
            ],
        };
        let registry = BTreeMap::from([
            ("Status".to_owned(), status.clone()),
            ("StatusPair".to_owned(), pair),
        ]);
        let value = ExecutionValue::Record {
            type_identity: sid("T:StatusPair"),
            name: "StatusPair".to_owned(),
            fields: vec![
                (
                    "left".to_owned(),
                    ExecutionValue::Finite {
                        type_identity: sid("T:Status"),
                        variant_identity: sid("V:FAIL"),
                        discriminant: 1,
                        payload: vec![],
                    },
                ),
                (
                    "right".to_owned(),
                    ExecutionValue::Finite {
                        type_identity: sid("T:Status"),
                        variant_identity: sid("V:FAIL"),
                        discriminant: 1,
                        payload: vec![],
                    },
                ),
            ],
        };
        let mut writer = ArenaWriter::new(registry);
        let boundary = writer.encode_argument(&value).expect("encodes");
        match boundary {
            BoundaryValue::Cell(root) => {
                assert_eq!(root, 0, "first cell sits at offset zero");
                let image = writer.into_image();
                assert_eq!(image.len(), 16, "two W32 slots");
                assert_eq!(u32::from_le_bytes(image[0..4].try_into().unwrap()), 1);
                assert_eq!(u32::from_le_bytes(image[8..12].try_into().unwrap()), 1);
            }
            other => panic!("record argument must cross as a cell, got {other:?}"),
        }
        // Round-trip through the reader.
        let image = {
            let mut w = ArenaWriter::new(BTreeMap::from([
                ("Status".to_owned(), status),
                (
                    "StatusPair".to_owned(),
                    BackendValueContract::Record {
                        type_identity: sid("T:StatusPair"),
                        name: "StatusPair".to_owned(),
                        fields: vec![
                            ("left".to_owned(), "Status".to_owned()),
                            ("right".to_owned(), "Status".to_owned()),
                        ],
                    },
                ),
            ]));
            let _ = w.encode_argument(&value).unwrap();
            w.into_image()
        };
        let empty = BTreeMap::new();
        let reader = ArenaReader::new(&image, &empty);
        let pair_contract = BackendValueContract::Record {
            type_identity: sid("T:StatusPair"),
            name: "StatusPair".to_owned(),
            fields: vec![
                ("left".to_owned(), "Status".to_owned()),
                ("right".to_owned(), "Status".to_owned()),
            ],
        };
        // Without a Status contract the reader cannot decode fields; with
        // one it must round-trip both discriminants.
        assert!(reader.decode(0, &pair_contract).is_err());
        let status_c = BackendValueContract::Finite {
            type_identity: sid("T:Status"),
            variants: BTreeMap::from([
                (0, sid("V:PASS")),
                (1, sid("V:FAIL")),
                (2, sid("V:UNKNOWN")),
            ]),
            payloads: BTreeMap::new(),
        };
        let registry2 = BTreeMap::from([("Status".to_owned(), status_c)]);
        let reader2 = ArenaReader::new(&image, &registry2);
        let decoded = reader2.decode(0, &pair_contract).expect("round-trips");
        let mncs_model::ExecutionValue::Record { fields, .. } = decoded else {
            panic!("decodes to a record");
        };
        for (_name, field) in fields {
            let mncs_model::ExecutionValue::Finite { discriminant, .. } = field else {
                panic!("field decodes to a finite");
            };
            assert_eq!(discriminant, 1);
        }
    }
}
