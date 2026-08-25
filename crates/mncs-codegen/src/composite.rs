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

impl SlotWidth {
    pub fn bytes(self) -> u64 {
        match self {
            Self::W32 => 4,
            Self::W64 => 8,
        }
    }
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
            width: slot_width(semantic_type),
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
            layout.boxed_finites.insert(finite.identity.clone(), variants);
        }
        layout
    }

    pub fn is_boxed_finite(&self, type_identity: &SemanticId) -> bool {
        self.boxed_finites.contains_key(type_identity)
    }

    pub fn is_composite(&self, type_identity: &SemanticId) -> bool {
        self.records.contains_key(type_identity)
            || self.boxed_finites.contains_key(type_identity)
    }

    /// Total cell size in bytes for one record type.
    pub fn record_cell_bytes(&self, type_identity: &SemanticId) -> Option<u64> {
        self.records
            .get(type_identity)
            .map(|fields| fields.len() as u64 * 8)
    }

    /// Total cell size in bytes for one boxed finite variant.
    pub fn finite_cell_bytes(&self, type_identity: &SemanticId, discriminant: u32) -> Option<u64> {
        self.boxed_finites
            .get(type_identity)?
            .get(&discriminant)
            .map(|fields| (fields.len() as u64 + 1) * 8)
    }
}

fn slot_width(semantic_type: &str) -> SlotWidth {
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

    pub fn image(&self) -> &[u8] {
        &self.image
    }

    fn align8(&mut self) -> u64 {
        let misalign = self.image.len() as u64 % 8;
        if misalign != 0 {
            self.image.resize(self.image.len() + (8 - misalign) as usize, 0);
        }
        self.image.len() as u64
    }

    fn put32(&mut self, offset: u64, value: u32) {
        self.image[offset as usize..offset as usize + 4]
            .copy_from_slice(&value.to_le_bytes());
    }

    fn put64(&mut self, offset: u64, value: u64) {
        self.image[offset as usize..offset as usize + 8]
            .copy_from_slice(&value.to_le_bytes());
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
                } if identity == type_identity && !payloads.is_empty()
            )
        })
    }

    fn named_contract(
        &self,
        declared_type: &str,
    ) -> Option<&mncs_model::BackendValueContract> {
        Self::resolve_contract(declared_type, &self.registry)
    }

    fn resolve_contract<'r>(
        declared_type: &str,
        registry: &'r BTreeMap<String, mncs_model::BackendValueContract>,
    ) -> Option<&'r mncs_model::BackendValueContract> {
        match mncs_model::BodyType::from_semantic_name(declared_type) {
            mncs_model::BodyType::Integer(_) => None,
            mncs_model::BodyType::Named(name) if name == "bool" => None,
            _ => registry.get(declared_type),
        }
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
                    return Err(
                        "record field count does not match the declared layout".to_owned()
                    );
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
                    self.store_field(
                        base + index as u64 * 8,
                        field_value,
                        declared_type,
                    )?;
                }
                Ok(base)
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
                    return Err(
                        "payload field count does not match the declared layout".to_owned()
                    );
                }
                self.image.resize(base as usize + (variant_fields.len() + 1) * 8, 0);
                self.put32(base, *discriminant);
                for (index, ((name, declared_type), (field_name, field_value))) in
                    variant_fields.iter().zip(payload.iter()).enumerate()
                {
                    if name != field_name {
                        return Err(format!(
                            "payload field {field_name:?} does not match the canonical layout {name:?}"
                        ));
                    }
                    self.store_field(
                        base + (index as u64 + 1) * 8,
                        field_value,
                        declared_type,
                    )?;
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
        let width = slot_width(declared_type);
        match value {
            Value::Record { .. } | Value::Finite { .. }
                if self.named_contract(declared_type).is_some() =>
            {
                let cell = self.encode_cell(value)?;
                self.put64(offset, cell);
                Ok(())
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
            Value::Finite {
                discriminant,
                payload,
                ..
            } if payload.is_empty() => {
                self.put32(offset, *discriminant);
                Ok(())
            }
            other => Err(format!(
                "nested composite field lacks a declared layout for encoding: {other:?}"
            )),
        }
    }
}

fn value_record_contract<'a>(
    value: &mncs_model::ExecutionValue,
    registry: &'a BTreeMap<String, mncs_model::BackendValueContract>,
) -> Option<&'a mncs_model::BackendValueContract> {
    let mncs_model::ExecutionValue::Record {
        type_identity, ..
    } = value
    else {
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
    registry.values().find_map(|contract| {
        match contract {
            mncs_model::BackendValueContract::Finite {
                type_identity: identity,
                payloads,
                ..
            } if identity == type_identity => payloads.get(discriminant),
            _ => None,
        }
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
            Contract::Scalar { .. } => Err("scalar results decode outside the cell codec".to_owned()),
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
        use mncs_model::BodyType;
        if let Some(contract) = Self::resolve_contract(declared_type, self.registry) {
            let reference = self.get64(offset)?;
            return self.decode(reference, contract);
        }
        match BodyType::from_semantic_name(declared_type) {
            BodyType::Integer(ty) => {
                let value: i128 = match slot_width(declared_type) {
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
