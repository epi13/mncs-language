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

use mncs_model::{IntegerType, Program, SemanticId};

/// Reconstruct a logical integer from a stored slot bit pattern.
/// Unsigned values keep their full domain; signed values sign-extend
/// from the declared width. Never route unsigned 64-bit cells through `i64`.
pub(crate) fn integer_from_slot_bits(raw: u64, ty: IntegerType) -> i128 {
    if ty.signed {
        match ty.bits {
            64 => raw as i64 as i128,
            32 => i128::from(raw as u32 as i32),
            16 => i128::from(raw as u16 as i16),
            8 => i128::from(raw as u8 as i8),
            bits => {
                let shift = 64 - bits.min(64);
                i128::from((raw as i64) << shift >> shift)
            }
        }
    } else if ty.bits >= 64 {
        i128::from(raw)
    } else {
        i128::from(raw & ((1_u64 << ty.bits) - 1))
    }
}

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
        mncs_model::BodyType::Sequence { .. }
        | mncs_model::BodyType::Vector { .. }
        | mncs_model::BodyType::Mask { .. } => SlotWidth::W64,
        _ => SlotWidth::W32,
    }
}

fn slot_width_registry(
    semantic_type: &str,
    registry: &BTreeMap<String, mncs_model::BackendValueContract>,
) -> SlotWidth {
    if let Some(contract) = registry.get(semantic_type) {
        match contract {
            mncs_model::BackendValueContract::Record { .. }
            | mncs_model::BackendValueContract::Sequence { .. }
            | mncs_model::BackendValueContract::View { .. }
            | mncs_model::BackendValueContract::Vector { .. }
            | mncs_model::BackendValueContract::Mask { .. } => return SlotWidth::W64,
            mncs_model::BackendValueContract::Finite { payloads, .. } => {
                return if finite_payloads_declare_payloads(payloads) {
                    SlotWidth::W64
                } else {
                    SlotWidth::W32
                };
            }
            mncs_model::BackendValueContract::Scalar { .. } => {}
        }
    }
    match mncs_model::BodyType::from_semantic_name(semantic_type) {
        mncs_model::BodyType::Integer(ty) if ty.bits == 64 => SlotWidth::W64,
        mncs_model::BodyType::Sequence { .. }
        | mncs_model::BodyType::Vector { .. }
        | mncs_model::BodyType::Mask { .. } => SlotWidth::W64,
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

/// Packed view descriptor used by every executable backend: the low 32 bits
/// hold the element-cell offset, the high 32 bits the runtime length.
pub fn pack_view(offset: u64, length: u32) -> u64 {
    (offset & 0xffff_ffff) | ((u64::from(length)) << 32)
}

pub fn unpack_view(descriptor: u64) -> (u64, u32) {
    (descriptor & 0xffff_ffff, (descriptor >> 32) as u32)
}

pub fn pack_mask(lanes: &[bool]) -> u64 {
    lanes.iter().enumerate().fold(
        0_u64,
        |acc, (index, bit)| {
            if *bit {
                acc | (1_u64 << index)
            } else {
                acc
            }
        },
    )
}

pub fn unpack_mask(bits: u64, lanes: u32) -> mncs_model::ExecutionValue {
    mncs_model::ExecutionValue::Mask {
        lanes: (0..lanes)
            .map(|index| ((bits >> index) & 1) == 1)
            .collect::<Vec<_>>()
            .into(),
    }
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
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn encode_argument(
        &mut self,
        value: &mncs_model::ExecutionValue,
    ) -> Result<BoundaryValue, String> {
        self.encode_argument_with_contract(value, None)
    }

    /// Encode one argument against its declared language-owned contract so
    /// exact sequences, views, vectors, and masks take the correct ABI word.
    pub fn encode_argument_with_contract(
        &mut self,
        value: &mncs_model::ExecutionValue,
        contract: Option<&mncs_model::BackendValueContract>,
    ) -> Result<BoundaryValue, String> {
        use mncs_model::{BackendValueContract as Contract, ExecutionValue as Value};
        match (contract, value) {
            (Some(Contract::Mask { lanes, .. }), Value::Mask { lanes: bits }) => {
                if bits.len() != *lanes as usize {
                    return Err("mask argument does not match its declared lane count".to_owned());
                }
                Ok(BoundaryValue::Bits(pack_mask(bits)))
            }
            (
                Some(Contract::View {
                    capacity, element, ..
                }),
                Value::Sequence { values },
            ) => {
                if values.len() > *capacity as usize {
                    return Err("view argument exceeds its declared capacity".to_owned());
                }
                if values.is_empty() {
                    return Ok(BoundaryValue::Bits(pack_view(0, 0)));
                }
                let root = self.encode_sequence_typed(values, element)?;
                Ok(BoundaryValue::Bits(pack_view(
                    root,
                    u32::try_from(values.len()).unwrap_or(0),
                )))
            }
            (
                Some(Contract::Sequence {
                    length, element, ..
                }),
                Value::Sequence { values },
            ) => {
                if values.len() != *length as usize {
                    return Err("sequence argument does not match its declared length".to_owned());
                }
                Ok(BoundaryValue::Cell(
                    self.encode_sequence_typed(values, element)?,
                ))
            }
            (Some(Contract::Vector { lanes, element, .. }), Value::Vector { values }) => {
                if values.len() != *lanes as usize {
                    return Err("vector argument does not match its declared lane count".to_owned());
                }
                Ok(BoundaryValue::Cell(
                    self.encode_vector_native(values, element)?,
                ))
            }
            (_, Value::Integer { value, .. }) => Ok(BoundaryValue::Bits(*value as u64)),
            (_, Value::Boolean { value }) => Ok(BoundaryValue::Bits(u64::from(*value))),
            (_, Value::Byte { value }) => Ok(BoundaryValue::Bits(*value as u64)),
            (
                _,
                Value::Finite {
                    payload,
                    discriminant,
                    type_identity,
                    ..
                },
            ) if !self.identity_is_boxed(type_identity) && payload.is_empty() => {
                Ok(BoundaryValue::Bits(u64::from(*discriminant)))
            }
            (_, Value::Mask { lanes }) => Ok(BoundaryValue::Bits(pack_mask(lanes))),
            (_, other) => Ok(BoundaryValue::Cell(self.encode_cell(other)?)),
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
                self.encode_sequence_typed(&elements, inferred_element_type(&elements))
            }
            Value::Vector { values } => {
                let elements = values.clone();
                self.encode_vector_native(&elements, inferred_element_type(&elements))
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
            Value::Sequence { values } => {
                let (element, view_capacity) =
                    match mncs_model::BodyType::from_semantic_name(declared_type) {
                        mncs_model::BodyType::Sequence {
                            element,
                            bound: mncs_model::SequenceBound::UpTo(capacity),
                        } => (element.semantic_name(), Some(capacity)),
                        mncs_model::BodyType::Sequence { element, .. } => {
                            (element.semantic_name(), None)
                        }
                        _ => (inferred_element_type(values).to_owned(), None),
                    };
                if let Some(capacity) = view_capacity {
                    if values.len() > capacity as usize {
                        return Err("view field exceeds its declared capacity".to_owned());
                    }
                    if values.is_empty() {
                        self.put64(offset, pack_view(0, 0));
                        return Ok(());
                    }
                    let cell = self.encode_sequence_typed(values, &element)?;
                    self.put64(
                        offset,
                        pack_view(cell, u32::try_from(values.len()).unwrap_or(0)),
                    );
                    return Ok(());
                }
                let cell = self.encode_sequence_typed(values, &element)?;
                self.put64(offset, cell);
                Ok(())
            }
            Value::Vector { values } => {
                let element = match mncs_model::BodyType::from_semantic_name(declared_type) {
                    mncs_model::BodyType::Vector { element, .. } => element.semantic_name(),
                    _ => inferred_element_type(values).to_owned(),
                };
                let cell = self.encode_vector_native(values, &element)?;
                self.put64(offset, cell);
                Ok(())
            }
            Value::Mask { lanes } => {
                self.put64(offset, pack_mask(lanes));
                Ok(())
            }
        }
    }

    /// Encode an exact-length sequence into a fresh canonical cell: one
    /// 8-byte slot per element in index order. Returns the cell offset.
    fn encode_sequence_typed(
        &mut self,
        elements: &[mncs_model::ExecutionValue],
        element_type: &str,
    ) -> Result<u64, String> {
        let base = self.align8();
        self.image.resize(base as usize + elements.len() * 8, 0);
        for (index, element) in elements.iter().enumerate() {
            self.store_field(base + index as u64 * 8, element, element_type)?;
        }
        Ok(base)
    }

    /// Native scalar backends store one 8-byte slot per vector lane.
    fn encode_vector_native(
        &mut self,
        lanes: &[mncs_model::ExecutionValue],
        element_type: &str,
    ) -> Result<u64, String> {
        self.encode_sequence_typed(lanes, element_type)
    }
}

fn inferred_element_type(values: &[mncs_model::ExecutionValue]) -> &'static str {
    match values.first() {
        Some(mncs_model::ExecutionValue::Byte { .. }) => "byte",
        Some(mncs_model::ExecutionValue::Boolean { .. }) => "bool",
        Some(mncs_model::ExecutionValue::Integer { ty, .. }) => {
            if ty.bits == 32 && ty.signed {
                "i32"
            } else if ty.bits == 32 && !ty.signed {
                "u32"
            } else if ty.bits == 16 && ty.signed {
                "i16"
            } else if ty.bits == 16 && !ty.signed {
                "u16"
            } else if ty.bits == 8 && !ty.signed {
                "u8"
            } else if !ty.signed {
                "u64"
            } else {
                "i64"
            }
        }
        _ => "i64",
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

    /// Decode one ABI word (`root` is a cell offset, packed view, or mask
    /// bits) against the declared contract.
    pub fn decode_boundary(
        &self,
        word: u64,
        contract: &mncs_model::BackendValueContract,
    ) -> Result<mncs_model::ExecutionValue, String> {
        use mncs_model::BackendValueContract as Contract;
        match contract {
            Contract::View {
                element, capacity, ..
            } => {
                let (offset, length) = unpack_view(word);
                if length > *capacity {
                    return Err("decoded view length exceeds its declared capacity".to_owned());
                }
                self.decode_sequence_at(offset, length, element)
            }
            Contract::Mask { lanes, .. } => Ok(unpack_mask(word, *lanes)),
            other => self.decode(word, other),
        }
    }

    /// Decode one value at `root` against a named declared semantic type.
    pub fn decode(
        &self,
        root: u64,
        contract: &mncs_model::BackendValueContract,
    ) -> Result<mncs_model::ExecutionValue, String> {
        use mncs_model::{BackendValueContract as Contract, ExecutionValue as Value};
        match contract {
            Contract::Scalar { .. } | Contract::Mask { .. } | Contract::View { .. } => {
                Err("scalar, mask, and view results decode outside a bare cell root".to_owned())
            }
            Contract::Sequence {
                element, length, ..
            } => self.decode_sequence_at(root, *length, element),
            Contract::Vector { element, lanes, .. } => self
                .decode_sequence_at(root, *lanes, element)
                .map(|decoded| {
                    let Value::Sequence { values } = decoded else {
                        return decoded;
                    };
                    Value::Vector { values }
                }),
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
                    fields: decoded.into(),
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
                    payload: payload.into(),
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
            // A payload map contains an entry for every declared variant, so
            // its mere presence does not mean the finite is boxed. Only a
            // payload-bearing variant crosses as a cell reference; a bare
            // finite is stored directly in the record slot.
            if finite_payloads_declare_payloads(payloads) {
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
                payload: Vec::new().into(),
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
                let raw = match slot_width_registry(declared_type, self.registry) {
                    SlotWidth::W32 => u64::from(self.get32(offset)?),
                    SlotWidth::W64 => self.get64(offset)?,
                };
                Ok(mncs_model::ExecutionValue::Integer {
                    value: integer_from_slot_bits(raw, ty),
                    ty,
                })
            }
            BodyType::Named(name) if name == "bool" => Ok(mncs_model::ExecutionValue::Boolean {
                value: self.get32(offset)? == 1,
            }),
            BodyType::Byte => Ok(mncs_model::ExecutionValue::Byte {
                value: i128::from(self.get32(offset)?),
            }),
            BodyType::Sequence {
                element,
                bound: mncs_model::SequenceBound::Exact(length),
            } => {
                let reference = self.get64(offset)?;
                let element_name = element.semantic_name();
                self.decode_sequence_at(reference, length, &element_name)
            }
            BodyType::Sequence {
                element,
                bound: mncs_model::SequenceBound::UpTo(capacity),
            } => {
                let descriptor = self.get64(offset)?;
                let (cell, length) = unpack_view(descriptor);
                if length > capacity {
                    return Err("decoded view length exceeds its declared capacity".to_owned());
                }
                let element_name = element.semantic_name();
                self.decode_sequence_at(cell, length, &element_name)
            }
            BodyType::Vector { element, lanes } => {
                let reference = self.get64(offset)?;
                let element_name = element.semantic_name();
                match self.decode_sequence_at(reference, lanes, &element_name)? {
                    mncs_model::ExecutionValue::Sequence { values } => {
                        Ok(mncs_model::ExecutionValue::Vector { values })
                    }
                    other => Ok(other),
                }
            }
            BodyType::Mask { lanes } => Ok(unpack_mask(self.get64(offset)?, lanes)),
            _ => Err(format!(
                "field type {declared_type} has no contract for decoding"
            )),
        }
    }

    fn decode_sequence_at(
        &self,
        root: u64,
        length: u32,
        element: &str,
    ) -> Result<mncs_model::ExecutionValue, String> {
        let mut values = Vec::with_capacity(length as usize);
        for index in 0..length {
            values.push(self.decode_field(root + u64::from(index) * 8, element)?);
        }
        Ok(mncs_model::ExecutionValue::Sequence {
            values: values.into(),
        })
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
            payloads: BTreeMap::from([(0, Vec::new()), (1, Vec::new()), (2, Vec::new())]),
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
                        payload: vec![].into(),
                    },
                ),
                (
                    "right".to_owned(),
                    ExecutionValue::Finite {
                        type_identity: sid("T:Status"),
                        variant_identity: sid("V:FAIL"),
                        discriminant: 1,
                        payload: vec![].into(),
                    },
                ),
            ]
            .into(),
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
            payloads: BTreeMap::from([(0, Vec::new()), (1, Vec::new()), (2, Vec::new())]),
        };
        let registry2 = BTreeMap::from([("Status".to_owned(), status_c)]);
        let reader2 = ArenaReader::new(&image, &registry2);
        let decoded = reader2.decode(0, &pair_contract).expect("round-trips");
        let mncs_model::ExecutionValue::Record { fields, .. } = decoded else {
            panic!("decodes to a record");
        };
        for (_name, field) in fields.as_ref() {
            let mncs_model::ExecutionValue::Finite { discriminant, .. } = field else {
                panic!("field decodes to a finite");
            };
            assert_eq!(*discriminant, 1);
        }
    }

    #[test]
    fn exact_byte_sequence_round_trips_through_the_canonical_cell() {
        let contract = BackendValueContract::Sequence {
            semantic_type: "[byte; 4]".to_owned(),
            element: "byte".to_owned(),
            length: 4,
        };
        let value = ExecutionValue::Sequence {
            values: vec![
                ExecutionValue::Byte { value: 0 },
                ExecutionValue::Byte { value: 0 },
                ExecutionValue::Byte { value: 4 },
                ExecutionValue::Byte { value: 0 },
            ]
            .into(),
        };
        let mut writer = ArenaWriter::new(BTreeMap::new());
        let boundary = writer
            .encode_argument_with_contract(&value, Some(&contract))
            .expect("encodes");
        let BoundaryValue::Cell(root) = boundary else {
            panic!("exact sequence crosses as a cell");
        };
        let image = writer.into_image();
        let registry = BTreeMap::new();
        let reader = ArenaReader::new(&image, &registry);
        let decoded = reader.decode(root, &contract).expect("decodes");
        assert_eq!(decoded, value);
    }

    #[test]
    fn view_descriptor_packs_offset_and_length() {
        assert_eq!(pack_view(16, 3), 16 | (3_u64 << 32));
        assert_eq!(unpack_view(16 | (3_u64 << 32)), (16, 3));
    }

    #[test]
    fn unsigned_u64_slot_bits_keep_the_high_bit() {
        let ty = IntegerType {
            bits: 64,
            signed: false,
        };
        assert_eq!(integer_from_slot_bits(u64::MAX, ty), i128::from(u64::MAX));
        assert_eq!(
            integer_from_slot_bits(1_u64 << 63, ty),
            9_223_372_036_854_775_808
        );
        assert_eq!(
            integer_from_slot_bits((1_u64 << 63) + 1, ty),
            9_223_372_036_854_775_809
        );
        assert_eq!(integer_from_slot_bits(7, ty), 7);
        let signed = IntegerType {
            bits: 64,
            signed: true,
        };
        assert_eq!(
            integer_from_slot_bits(1_u64 << 63, signed),
            i64::MIN as i128
        );
    }

    #[test]
    fn exact_u64_sequence_preserves_high_bit_values() {
        let contract = BackendValueContract::Sequence {
            semantic_type: "[u64; 4]".to_owned(),
            element: "u64".to_owned(),
            length: 4,
        };
        let u64_ty = IntegerType {
            bits: 64,
            signed: false,
        };
        let value = ExecutionValue::Sequence {
            values: vec![
                ExecutionValue::Integer {
                    value: i128::from(u64::MAX),
                    ty: u64_ty,
                },
                ExecutionValue::Integer {
                    value: 1_i128 << 63,
                    ty: u64_ty,
                },
                ExecutionValue::Integer {
                    value: (1_i128 << 63) + 1,
                    ty: u64_ty,
                },
                ExecutionValue::Integer {
                    value: 7,
                    ty: u64_ty,
                },
            ]
            .into(),
        };
        let mut writer = ArenaWriter::new(BTreeMap::new());
        let boundary = writer
            .encode_argument_with_contract(&value, Some(&contract))
            .expect("encodes");
        let BoundaryValue::Cell(root) = boundary else {
            panic!("exact sequence crosses as a cell");
        };
        let image = writer.into_image();
        let registry = BTreeMap::new();
        let reader = ArenaReader::new(&image, &registry);
        let decoded = reader.decode(root, &contract).expect("decodes");
        assert_eq!(decoded, value);
    }
}
