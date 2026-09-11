//! Lower selected SSA into the portable WASM MVP subset.

use std::collections::{BTreeMap, BTreeSet};

use mncs_model::{
    ArithmeticIntent, BodyType, IntegerType, IrType, SemanticId, SsaFunction, SsaInstructionKind,
    SsaModule, SsaTerminator, SsaValue,
};

use crate::composite::SlotWidth;
use crate::wasm::{Instr, ValType, WasmFunction, WasmImport, WasmModule};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweringOutcome {
    pub module: Option<WasmModule>,
    pub exports: Vec<String>,
    pub unsupported: Vec<String>,
}

struct FunctionLayout {
    values: BTreeMap<SemanticId, u32>,
    types: BTreeMap<SemanticId, ValType>,
    pc: u32,
    blocks: BTreeMap<SemanticId, u32>,
    block_params: BTreeMap<SemanticId, Vec<SemanticId>>,
    functions: BTreeMap<SemanticId, u32>,
}

// ---------------------------------------------------------------------------
// WEB-P-012 loop-region reclamation.
//
// Native arenas are bump-only: every functional aggregate update allocates
// a fresh cell, so a bounded loop carrying a large value allocates
// (trips x cell-bytes) total and exhausts any fixed arena. Loop regions
// bound the peak instead: the first header visit of a loop activation
// records the bump as the region mark, and each backedge copies the
// header's live cells aside into locals (flatten), resets the bump to
// the mark (discarding the iteration's garbage AND the previous trip's
// rebuilt duplicates together), and rebuilds the values in the parent
// region (unflatten). Exits deactivate the region without resetting, so
// live-out cells stay valid where they are and the next entry pushes a
// fresh mark.
//
// Soundness rests on three language-owned facts:
//
// * Cells are immutable values, so duplicating a cell preserves meaning.
// * Past the backedge's argument move, the post-move locals that
//   downstream reads can observe are exactly the header's parameters
//   (rebound to the new cells) plus the other values live at the header
//   entry. Copying every cell there keeps every future read valid; words
//   ride in locals untouched. Latch argument values need no copying:
//   dominance guarantees they are rewritten in the latch before any
//   post-reset read.
// * The mark is pushed once per activation (guarded by a per-region
//   active flag set at the header, cleared on every loop exit and at
//   function entry), and only single-entry natural loops are planned, so
//   a reset can only ever target a mark pushed after every cell it frees
//   was allocated: everything above the mark at a backedge is either
//   dead or rebuilt from scratch temps first.
//
// A loop is skipped (status-quo bump behavior, always sound) when any of
// this fails to establish: a non-single-entry body, unresolvable nominal
// types, live views (a view descriptor cannot be rebased without its
// source base, so view-carrying loops keep today's behavior), values
// missing from the layout, or excessive static copy size. The backedge
// sequence is additionally guarded on the host-buffer end cursor when the
// module has one: a host reservation inside the loop means the mark no
// longer bounds scratch, so that iteration keeps status-quo behavior
// with all locals valid.

/// Upper bound on flatten/unflatten scratch words per loop region. Larger
/// static live sets skip reclamation (sound fallback to bump behavior);
/// the bound keeps generated code proportional to the program.
const REGION_TEMP_CAP_WORDS: u32 = 65_536;

/// One nominal field fact for region copy-out: field name, slot width,
/// and whether the slot holds a nested cell reference (records, boxed
/// finites, exact sequences, vectors) rather than an inline word.
type NominalField = (String, crate::composite::SlotWidth, bool);

/// Nominal field facts for region copy-out, keyed by type identity (and
/// discriminant for finite variants).
#[derive(Debug, Clone, Default)]
struct NominalRefMap {
    records: BTreeMap<SemanticId, Vec<NominalField>>,
    finites: BTreeMap<SemanticId, BTreeMap<u32, Vec<NominalField>>>,
}

impl NominalRefMap {
    fn from_program(program: &mncs_model::Program, composites: &CompositeInfo) -> Self {
        let is_ref = |field_type: &str| -> bool {
            if program
                .record_types
                .iter()
                .any(|record| record.name == field_type || record.identity.0 == field_type)
            {
                return true;
            }
            if program
                .finite_types
                .iter()
                .any(|finite| finite.name == field_type || finite.identity.0 == field_type)
            {
                return true;
            }
            matches!(
                BodyType::from_semantic_name(field_type),
                BodyType::Sequence {
                    bound: mncs_model::SequenceBound::Exact(_),
                    ..
                } | BodyType::Vector { .. }
            )
        };
        let mut map = Self::default();
        // Slot order follows the canonical composite layout (the same order
        // lowering stores); field spellings resolve from the declaration by
        // name, so layout/declaration order can never drift apart here.
        for record in &program.record_types {
            let empty = Vec::new();
            let layout = composites.records.get(&record.identity).unwrap_or(&empty);
            let fields = layout
                .iter()
                .map(|(name, width)| {
                    let spelling = record
                        .fields
                        .iter()
                        .find(|field| &field.name == name)
                        .map(|field| field.field_type.as_str())
                        .unwrap_or("u64");
                    (name.clone(), *width, is_ref(spelling))
                })
                .collect::<Vec<_>>();
            map.records.insert(record.identity.clone(), fields);
        }
        for finite in &program.finite_types {
            // Tag-only finites are unboxed words, never cells: keep them
            // out of the map so shape resolution agrees with lowering.
            if !composites.is_boxed_finite(&finite.identity) {
                continue;
            }
            let mut variants = BTreeMap::new();
            for variant in &finite.variants {
                let empty = Vec::new();
                let layout = composites
                    .boxed_finites
                    .get(&finite.identity)
                    .and_then(|variants| variants.get(&variant.discriminant))
                    .unwrap_or(&empty);
                let fields = layout
                    .iter()
                    .map(|(name, width)| {
                        let spelling = variant
                            .payload
                            .iter()
                            .find(|field| &field.name == name)
                            .map(|field| field.field_type.as_str())
                            .unwrap_or("u64");
                        (name.clone(), *width, is_ref(spelling))
                    })
                    .collect::<Vec<_>>();
                variants.insert(variant.discriminant, fields);
            }
            map.finites.insert(finite.identity.clone(), variants);
        }
        map
    }
}

/// Copy shape for one carried/live-out value: words copy inline, cells
/// copy field-by-field with nested references recursed.
#[derive(Debug, Clone)]
enum CopyShape {
    /// One inline word (scalars, views, masks, unboxed tags).
    Word(crate::composite::SlotWidth),
    /// Canonical record cell: per-field (slot width, nested shape).
    Record(Vec<(crate::composite::SlotWidth, CopyShape)>),
    /// Boxed finite cell: discriminant word plus per-variant payloads.
    Finite(Vec<(u32, Vec<(crate::composite::SlotWidth, CopyShape)>)>),
    /// Exact sequence / vector cell: static lane count, lane stride, lane.
    Lanes {
        count: u32,
        stride_bytes: u32,
        element: Box<CopyShape>,
    },
}

/// Whether an SSA value type needs region copying: `Cell` shapes copy,
/// `Word` values ride in locals untouched, `View` carriers skip the loop.
#[derive(Debug, Clone)]
enum CopyClass {
    Word,
    Cell(CopyShape),
    View,
}

fn slot_width_of(spelling: &str) -> crate::composite::SlotWidth {
    match BodyType::from_semantic_name(spelling) {
        BodyType::Integer(ty) if ty.bits == 64 => crate::composite::SlotWidth::W64,
        BodyType::Float(ty) if ty.is_supported() => crate::composite::SlotWidth::W64,
        BodyType::Sequence {
            bound: mncs_model::SequenceBound::UpTo(_),
            ..
        }
        | BodyType::Mask { .. } => crate::composite::SlotWidth::W64,
        _ => crate::composite::SlotWidth::W32,
    }
}

/// Resolve the copy shape of one field/element spelling: named composites
/// recurse through the program declarations, structural spellings recurse
/// structurally, and everything else is an inline word. `View` reports a
/// bounded view at any depth (callers skip the loop); `Err` reports an
/// unresolvable spelling (callers skip the loop). `visiting` tracks the
/// in-progress declaration chain: a recursive type re-enters its own
/// spelling, and since no static temp assignment can flatten unbounded
/// nesting depth, that is an `Err` (skip) rather than unbounded host
/// recursion.
fn copy_shape_of_spelling(
    spelling: &str,
    program: &mncs_model::Program,
    refmap: &NominalRefMap,
    composites: &CompositeInfo,
    visiting: &mut BTreeSet<String>,
) -> Result<CopyClass, String> {
    // Normalize onto the declaration identity when the spelling names one:
    // the same type is reachable by name and by identity, and the guard
    // must catch both.
    let key = program
        .record_types
        .iter()
        .find(|record| record.name == spelling || record.identity.0 == spelling)
        .map(|record| record.identity.0.clone())
        .or_else(|| {
            program
                .finite_types
                .iter()
                .find(|finite| finite.name == spelling || finite.identity.0 == spelling)
                .map(|finite| finite.identity.0.clone())
        })
        .unwrap_or_else(|| spelling.to_owned());
    if !visiting.insert(key.clone()) {
        return Err(format!(
            "recursive type {spelling:?} has no bounded copy shape"
        ));
    }
    let resolved = copy_shape_of_spelling_inner(spelling, program, refmap, composites, visiting);
    visiting.remove(&key);
    resolved
}

fn copy_shape_of_spelling_inner(
    spelling: &str,
    program: &mncs_model::Program,
    refmap: &NominalRefMap,
    composites: &CompositeInfo,
    visiting: &mut BTreeSet<String>,
) -> Result<CopyClass, String> {
    if let Some(record) = program
        .record_types
        .iter()
        .find(|record| record.name == spelling || record.identity.0 == spelling)
    {
        let fields = refmap
            .records
            .get(&record.identity)
            .ok_or_else(|| format!("record {spelling:?} has no canonical layout"))?;
        if fields.len() != record.fields.len() {
            return Err(format!(
                "record {spelling:?} layout does not match its declaration"
            ));
        }
        let mut shape = Vec::with_capacity(fields.len());
        for (name, width, is_ref) in fields.iter() {
            let spelling = record
                .fields
                .iter()
                .find(|field| &field.name == name)
                .map(|field| field.field_type.as_str())
                .ok_or_else(|| format!("record field {name:?} missing from its declaration"))?;
            let nested = if *is_ref {
                match copy_shape_of_spelling(spelling, program, refmap, composites, visiting)? {
                    CopyClass::Cell(nested) => nested,
                    CopyClass::Word => {
                        return Err(format!(
                            "record field {name:?} marked as a reference but resolves to a word"
                        ));
                    }
                    CopyClass::View => return Ok(CopyClass::View),
                }
            } else {
                CopyShape::Word(*width)
            };
            shape.push((*width, nested));
        }
        return Ok(CopyClass::Cell(CopyShape::Record(shape)));
    }
    if let Some(finite) = program
        .finite_types
        .iter()
        .find(|finite| finite.name == spelling || finite.identity.0 == spelling)
    {
        // Tag-only finites are unboxed words, exactly as the lowering
        // realizes them (`CompositeInfo::is_boxed_finite` is the same
        // predicate the constructor/store paths use). Treating an unboxed
        // tag as a cell reference would flatten a discriminant as an
        // address and rebuild garbage.
        if !composites.is_boxed_finite(&finite.identity) {
            return Ok(CopyClass::Word);
        }
        let Some(variants) = refmap.finites.get(&finite.identity) else {
            return Err(format!("boxed finite {spelling:?} has no canonical layout"));
        };
        let mut shape = Vec::with_capacity(variants.len());
        for variant in &finite.variants {
            let layout = variants.get(&variant.discriminant).ok_or_else(|| {
                format!(
                    "finite variant {:?} has no canonical layout",
                    variant.discriminant
                )
            })?;
            if layout.len() != variant.payload.len() {
                return Err(format!(
                    "finite variant {:?} layout does not match its declaration",
                    variant.discriminant
                ));
            }
            let mut payload = Vec::with_capacity(layout.len());
            for (name, width, is_ref) in layout.iter() {
                let spelling = variant
                    .payload
                    .iter()
                    .find(|field| &field.name == name)
                    .map(|field| field.field_type.as_str())
                    .ok_or_else(|| {
                        format!("finite payload field {name:?} missing from its declaration")
                    })?;
                let nested = if *is_ref {
                    match copy_shape_of_spelling(spelling, program, refmap, composites, visiting)? {
                        CopyClass::Cell(nested) => nested,
                        CopyClass::Word => {
                            return Err(format!(
                                "finite payload field {name:?} marked as a reference but resolves to a word"
                            ));
                        }
                        CopyClass::View => return Ok(CopyClass::View),
                    }
                } else {
                    CopyShape::Word(*width)
                };
                payload.push((*width, nested));
            }
            shape.push((variant.discriminant, payload));
        }
        return Ok(CopyClass::Cell(CopyShape::Finite(shape)));
    }
    match BodyType::from_semantic_name(spelling) {
        BodyType::Sequence {
            element,
            bound: mncs_model::SequenceBound::Exact(length),
        } => {
            let element_spelling = element.semantic_name();
            let element = match copy_shape_of_spelling(
                &element_spelling,
                program,
                refmap,
                composites,
                visiting,
            )? {
                CopyClass::Cell(nested) => nested,
                // Scalar lanes copy as words with the lane width.
                CopyClass::Word => CopyShape::Word(slot_width_of(&element_spelling)),
                CopyClass::View => return Ok(CopyClass::View),
            };
            Ok(CopyClass::Cell(CopyShape::Lanes {
                count: length,
                stride_bytes: 8,
                element: Box::new(element),
            }))
        }
        BodyType::Sequence {
            bound: mncs_model::SequenceBound::UpTo(_),
            ..
        } => Ok(CopyClass::View),
        BodyType::Vector { element, lanes } => {
            let BodyType::Integer(integer) = *element else {
                return Err("vector lanes must be integers".to_owned());
            };
            let width = if integer.bits == 64 {
                crate::composite::SlotWidth::W64
            } else {
                crate::composite::SlotWidth::W32
            };
            Ok(CopyClass::Cell(CopyShape::Lanes {
                count: lanes,
                stride_bytes: if integer.bits == 64 { 8 } else { 4 },
                element: Box::new(CopyShape::Word(width)),
            }))
        }
        _ => Ok(CopyClass::Word),
    }
}

/// Classify one SSA block-parameter type for region copying.
fn copy_class_of_ty(
    ty: &IrType,
    program: &mncs_model::Program,
    refmap: &NominalRefMap,
    composites: &CompositeInfo,
) -> Result<CopyClass, String> {
    // Each classification is an independent declaration walk, so the
    // cycle guard starts empty here and only spans one shape.
    let mut visiting = BTreeSet::new();
    match ty {
        IrType::Record { name, .. } | IrType::Finite { name, .. } => {
            copy_shape_of_spelling(name, program, refmap, composites, &mut visiting)
        }
        IrType::Named(spelling) => {
            copy_shape_of_spelling(spelling, program, refmap, composites, &mut visiting)
        }
    }
}

/// Scratch words to flatten one shape aside (one I64 temp per word slot,
/// including one per nested reference hop).
fn flatten_words(shape: &CopyShape) -> u32 {
    match shape {
        CopyShape::Word(_) => 1,
        CopyShape::Record(fields) => fields
            .iter()
            .map(|(_, nested)| match nested {
                CopyShape::Word(_) => 1,
                nested => 1 + flatten_words(nested),
            })
            .sum(),
        CopyShape::Finite(variants) => {
            1 + variants
                .iter()
                .map(|(_, payload)| {
                    payload
                        .iter()
                        .map(|(_, nested)| match nested {
                            CopyShape::Word(_) => 1,
                            nested => 1 + flatten_words(nested),
                        })
                        .sum::<u32>()
                })
                .max()
                .unwrap_or(0)
        }
        CopyShape::Lanes { count, element, .. } => count.saturating_mul(match element.as_ref() {
            CopyShape::Word(_) => 1,
            nested => 1 + flatten_words(nested),
        }),
    }
}

/// Canonical cell bytes for one shape (what unflatten allocates).
fn cell_bytes(shape: &CopyShape) -> u32 {
    match shape {
        CopyShape::Word(_) => 0,
        CopyShape::Record(fields) => fields.len() as u32 * 8,
        CopyShape::Finite(variants) => variants
            .iter()
            .map(|(_, payload)| (payload.len() as u32 + 1) * 8)
            .max()
            .unwrap_or(8),
        CopyShape::Lanes {
            count,
            stride_bytes,
            ..
        } => count.saturating_mul(*stride_bytes),
    }
}

fn emit_push_slot(body: &mut Vec<Instr>, addr_local: u32, byte_offset: u32) {
    body.push(Instr::LocalGet(addr_local));
    if byte_offset != 0 {
        body.push(Instr::I32Const(byte_offset as i32));
        body.push(Instr::I32Add);
    }
}

fn emit_load_width(body: &mut Vec<Instr>, width: crate::composite::SlotWidth) {
    load_instr(width, body);
}

fn emit_extend_for_temp(body: &mut Vec<Instr>, width: crate::composite::SlotWidth) {
    // Scratch temps are uniformly I64 so one temp range serves every slot.
    if matches!(width, crate::composite::SlotWidth::W32) {
        body.push(Instr::I64ExtendI32U);
    }
}

/// Flatten one cell aside into `temp..`: read every slot (recursing into
/// nested references) without writing memory. Returns temps consumed; the
/// assignment is deterministic so unflatten reuses it exactly.
fn emit_flatten(body: &mut Vec<Instr>, addr_local: u32, shape: &CopyShape, temp: u32) -> u32 {
    match shape {
        CopyShape::Word(width) => {
            emit_push_slot(body, addr_local, 0);
            emit_load_width(body, *width);
            emit_extend_for_temp(body, *width);
            body.push(Instr::LocalSet(temp));
            1
        }
        CopyShape::Record(fields) => {
            let mut used = 0;
            for (index, (width, nested)) in fields.iter().enumerate() {
                let offset = index as u32 * 8;
                match nested {
                    CopyShape::Word(_) => {
                        emit_push_slot(body, addr_local, offset);
                        emit_load_width(body, *width);
                        emit_extend_for_temp(body, *width);
                        body.push(Instr::LocalSet(temp + used));
                        used += 1;
                    }
                    nested => {
                        emit_push_slot(body, addr_local, offset);
                        body.push(Instr::I32Load);
                        // References ride extended in the uniform I64
                        // scratch temps (a bare I32 into an I64 local
                        // would fail WASM validation).
                        body.push(Instr::I64ExtendI32U);
                        body.push(Instr::LocalSet(temp + used));
                        used += 1 + emit_flatten(body, temp + used, nested, temp + used + 1);
                    }
                }
            }
            used
        }
        CopyShape::Finite(variants) => {
            emit_push_slot(body, addr_local, 0);
            body.push(Instr::I32Load);
            body.push(Instr::I64ExtendI32U);
            body.push(Instr::LocalSet(temp));
            let mut max = 0;
            for (discriminant, payload) in variants {
                // The discriminant temp holds an extended I64 (see
                // above), so the tag compare is 64-bit.
                body.push(Instr::LocalGet(temp));
                body.push(Instr::I64Const(*discriminant as i64));
                body.push(Instr::I64Eq);
                body.push(Instr::If);
                let mut used = 0;
                for (index, (width, nested)) in payload.iter().enumerate() {
                    let offset = (index as u32 + 1) * 8;
                    match nested {
                        CopyShape::Word(_) => {
                            emit_push_slot(body, addr_local, offset);
                            emit_load_width(body, *width);
                            emit_extend_for_temp(body, *width);
                            body.push(Instr::LocalSet(temp + 1 + used));
                            used += 1;
                        }
                        nested => {
                            emit_push_slot(body, addr_local, offset);
                            body.push(Instr::I32Load);
                            body.push(Instr::I64ExtendI32U);
                            body.push(Instr::LocalSet(temp + 1 + used));
                            used += 1 + emit_flatten(
                                body,
                                temp + 1 + used,
                                nested,
                                temp + 1 + used + 1,
                            );
                        }
                    }
                }
                max = max.max(used);
                body.push(Instr::End);
            }
            1 + max
        }
        CopyShape::Lanes {
            count,
            stride_bytes,
            element,
        } => {
            let mut used = 0;
            for lane in 0..*count {
                let offset = lane.saturating_mul(*stride_bytes);
                match element.as_ref() {
                    CopyShape::Word(width) => {
                        emit_push_slot(body, addr_local, offset);
                        emit_load_width(body, *width);
                        emit_extend_for_temp(body, *width);
                        body.push(Instr::LocalSet(temp + used));
                        used += 1;
                    }
                    nested => {
                        emit_push_slot(body, addr_local, offset);
                        body.push(Instr::I32Load);
                        body.push(Instr::I64ExtendI32U);
                        body.push(Instr::LocalSet(temp + used));
                        used += 1 + emit_flatten(body, temp + used, nested, temp + used + 1);
                    }
                }
            }
            used
        }
    }
}

/// Deepest nested-reference depth of one shape (root cell is depth 0).
/// Sizes the shared I32 address-temp pool: one temp per depth level.
fn shape_depth(shape: &CopyShape) -> u32 {
    let nested_depth = |nested: &CopyShape| match nested {
        CopyShape::Word(_) => 0,
        nested => 1 + shape_depth(nested),
    };
    match shape {
        CopyShape::Word(_) => 0,
        CopyShape::Record(fields) => fields
            .iter()
            .map(|(_, nested)| nested_depth(nested))
            .max()
            .unwrap_or(0),
        CopyShape::Finite(variants) => variants
            .iter()
            .map(|(_, payload)| {
                payload
                    .iter()
                    .map(|(_, nested)| nested_depth(nested))
                    .max()
                    .unwrap_or(0)
            })
            .max()
            .unwrap_or(0),
        CopyShape::Lanes { element, .. } => nested_depth(element),
    }
}

/// Rebuild one cell from scratch temps: allocate in the parent region
/// (post-reset bump) and write every slot, recursing into nested
/// references. Data temps mirror `emit_flatten` exactly; child addresses
/// ride in a SEPARATE I32 temp pool (`addr_base + depth`) because data
/// temps are uniformly I64 and a rebuilt address is I32 sharing one
/// local for both would fail WASM validation.
fn emit_unflatten(
    body: &mut Vec<Instr>,
    addr_base: u32,
    shape: &CopyShape,
    depth: u32,
    data_temp: u32,
    dest_local: u32,
) -> Result<(), String> {
    match shape {
        CopyShape::Word(_) => {
            // Words never reach unflatten as roots (only cells are copied).
            body.push(Instr::Unreachable);
            Ok(())
        }
        CopyShape::Record(fields) => {
            emit_alloc(body, dest_local, cell_bytes(shape))?;
            let mut used = 0;
            for (index, (width, nested)) in fields.iter().enumerate() {
                let offset = index as u32 * 8;
                match nested {
                    CopyShape::Word(_) => {
                        emit_push_slot(body, dest_local, offset);
                        body.push(Instr::LocalGet(data_temp + used));
                        emit_unwrap_temp(body, *width);
                        store_width(*width, body);
                        used += 1;
                    }
                    nested => {
                        let addr = addr_base + depth;
                        body.push(Instr::LocalGet(data_temp + used));
                        body.push(Instr::I32WrapI64);
                        body.push(Instr::LocalSet(addr));
                        used += 1;
                        emit_unflatten(body, addr_base, nested, depth + 1, data_temp + used, addr)?;
                        emit_push_slot(body, dest_local, offset);
                        body.push(Instr::LocalGet(addr));
                        body.push(Instr::I32Store);
                        used += flatten_words(nested);
                    }
                }
            }
            Ok(())
        }
        CopyShape::Finite(variants) => {
            for (discriminant, payload) in variants {
                body.push(Instr::LocalGet(data_temp));
                body.push(Instr::I64Const(*discriminant as i64));
                body.push(Instr::I64Eq);
                body.push(Instr::If);
                emit_alloc(body, dest_local, (payload.len() as u32 + 1) * 8)?;
                emit_push_slot(body, dest_local, 0);
                body.push(Instr::LocalGet(data_temp));
                body.push(Instr::I32WrapI64);
                body.push(Instr::I32Store);
                let mut used = 0;
                for (index, (width, nested)) in payload.iter().enumerate() {
                    let offset = (index as u32 + 1) * 8;
                    match nested {
                        CopyShape::Word(_) => {
                            emit_push_slot(body, dest_local, offset);
                            body.push(Instr::LocalGet(data_temp + 1 + used));
                            emit_unwrap_temp(body, *width);
                            store_width(*width, body);
                            used += 1;
                        }
                        nested => {
                            let addr = addr_base + depth;
                            body.push(Instr::LocalGet(data_temp + 1 + used));
                            body.push(Instr::I32WrapI64);
                            body.push(Instr::LocalSet(addr));
                            used += 1;
                            emit_unflatten(
                                body,
                                addr_base,
                                nested,
                                depth + 1,
                                data_temp + 1 + used,
                                addr,
                            )?;
                            emit_push_slot(body, dest_local, offset);
                            body.push(Instr::LocalGet(addr));
                            body.push(Instr::I32Store);
                            used += flatten_words(nested);
                        }
                    }
                }
                body.push(Instr::End);
            }
            Ok(())
        }
        CopyShape::Lanes {
            count,
            stride_bytes,
            element,
        } => {
            emit_alloc(body, dest_local, cell_bytes(shape))?;
            let mut used = 0;
            for lane in 0..*count {
                let offset = lane.saturating_mul(*stride_bytes);
                match element.as_ref() {
                    CopyShape::Word(width) => {
                        emit_push_slot(body, dest_local, offset);
                        body.push(Instr::LocalGet(data_temp + used));
                        emit_unwrap_temp(body, *width);
                        store_width(*width, body);
                        used += 1;
                    }
                    nested => {
                        let addr = addr_base + depth;
                        body.push(Instr::LocalGet(data_temp + used));
                        body.push(Instr::I32WrapI64);
                        body.push(Instr::LocalSet(addr));
                        used += 1;
                        emit_unflatten(body, addr_base, nested, depth + 1, data_temp + used, addr)?;
                        emit_push_slot(body, dest_local, offset);
                        body.push(Instr::LocalGet(addr));
                        body.push(Instr::I32Store);
                        used += flatten_words(nested);
                    }
                }
            }
            Ok(())
        }
    }
}

/// Narrow one I64 scratch temp back to a slot width for storing.
fn emit_unwrap_temp(body: &mut Vec<Instr>, width: crate::composite::SlotWidth) {
    if matches!(width, crate::composite::SlotWidth::W32) {
        body.push(Instr::I32WrapI64);
    }
}

/// One planned loop-region header: the bump mark global, the activation
/// flag global, and the cell values (WASM local + copy shape) to preserve
/// across the backedge reset. Sorted by local for deterministic emission.
struct HeaderPlan {
    mark_global: u32,
    active_global: u32,
    copies: Vec<(u32, CopyShape)>,
}

/// Per-function loop-region plan: planned headers by emission block
/// index, loop-exit edges by source block index (each entry names the
/// target block and the exited region's activation flag to clear), plus
/// the scratch pool sizes shared by all of the function's backedge
/// sequences (sequences run one at a time, so one pool sized to the
/// maximum serves every region).
#[derive(Default)]
struct RegionPlan {
    headers: BTreeMap<u32, HeaderPlan>,
    exits: BTreeMap<u32, Vec<(u32, u32)>>,
    addr_pool: u32,
    data_pool: u32,
}

/// Emission context for one function's backedge sequences.
struct RegionEmit<'a> {
    plan: &'a RegionPlan,
    addr_base: u32,
    data_base: u32,
    /// Host-buffer end global (`mncs_host_buffer` support), when the
    /// module materializes composites. `None` means no host reservation
    /// can exist, so the reset needs no guard.
    host_end: Option<u32>,
}

/// Plan loop-region reclamation for one function: find backedges (branch
/// targets at or before the source block in emission order), keep the
/// single-entry natural loops, compute order-sensitive backward liveness,
/// and preserve each planned header's parameters plus its other live-in
/// cell values across its backedge resets.
///
/// Soundness: after the backedge's argument move, downstream reads can
/// observe exactly the header's parameters (rebound) and the other
/// header-live values (untouched by the move). Copying every `Cell`
/// among them aside and rebuilding it after the reset keeps all future
/// reads valid because cells are immutable; words need no copying, and
/// latch temporaries need none by dominance. A header is left unplanned
/// (status-quo bump behavior, always sound) when its body is not
/// single-entry, when any preserved value is a view carrier (a view
/// descriptor cannot be rebased without its source base), has an
/// unresolvable shape, or is missing from the layout, or when the static
/// scratch need exceeds the temp cap.
fn plan_loop_regions(
    program: &mncs_model::Program,
    function: &SsaFunction,
    layout: &FunctionLayout,
    composites: &CompositeInfo,
    refmap: &NominalRefMap,
) -> Result<RegionPlan, String> {
    let mut plan = RegionPlan::default();
    if !composites.uses_composites {
        return Ok(plan);
    }
    let block_count = function.blocks.len();
    // Value types for classification: every live value is an input, a
    // block parameter, an instruction output, or the function result.
    let mut tys: BTreeMap<SemanticId, IrType> = BTreeMap::new();
    for input in &function.inputs {
        tys.insert(input.identity.clone(), input.ty.clone());
    }
    for block in &function.blocks {
        for parameter in &block.parameters {
            tys.insert(parameter.identity.clone(), parameter.ty.clone());
        }
        for instruction in &block.instructions {
            for output in &instruction.outputs {
                tys.insert(output.identity.clone(), output.ty.clone());
            }
        }
    }
    for output in &function.outputs {
        if let Some(ty) = tys.get(&output.identity).cloned() {
            tys.insert(output.identity.clone(), ty);
        }
    }
    // CFG successors with the edge's argument bindings. Failure edges
    // are not lowered (lower_terminator traps), so they contribute no
    // successors and no reads.
    let mut successors: Vec<Vec<(usize, Vec<SemanticId>)>> = vec![Vec::new(); block_count];
    for (index, block) in function.blocks.iter().enumerate() {
        match &block.terminator {
            SsaTerminator::Return { .. } => {}
            SsaTerminator::Branch { target, arguments } => {
                plan_edge(
                    layout,
                    block_count,
                    &mut successors,
                    index,
                    target,
                    arguments,
                )?;
            }
            SsaTerminator::ConditionalBranch {
                then_target,
                then_arguments,
                else_target,
                else_arguments,
                ..
            } => {
                plan_edge(
                    layout,
                    block_count,
                    &mut successors,
                    index,
                    then_target,
                    then_arguments,
                )?;
                plan_edge(
                    layout,
                    block_count,
                    &mut successors,
                    index,
                    else_target,
                    else_arguments,
                )?;
            }
            SsaTerminator::Failure { .. } => {}
        }
    }
    // Order-sensitive per-block uses (operands read before any in-block
    // write) and defs (params bound at entry, outputs written in order).
    // A value defined and then used inside one block is NOT live at the
    // block's entry; ignoring intra-block order would leak dead-in-block
    // values into every loop live set through cyclic propagation.
    let mut uses: Vec<BTreeSet<SemanticId>> = vec![BTreeSet::new(); block_count];
    let mut defs: Vec<BTreeSet<SemanticId>> = vec![BTreeSet::new(); block_count];
    for (index, block) in function.blocks.iter().enumerate() {
        let mut defined = BTreeSet::new();
        for parameter in &block.parameters {
            defined.insert(parameter.identity.clone());
        }
        let mut read_before_write = BTreeSet::new();
        for instruction in &block.instructions {
            for input in &instruction.inputs {
                if !defined.contains(input) {
                    read_before_write.insert(input.clone());
                }
            }
            for output in &instruction.outputs {
                defined.insert(output.identity.clone());
            }
        }
        let mut terminator_reads = Vec::new();
        match &block.terminator {
            SsaTerminator::Return { values } => {
                terminator_reads.extend(values.iter().cloned());
            }
            SsaTerminator::Branch { arguments, .. } => {
                terminator_reads.extend(arguments.iter().cloned());
            }
            SsaTerminator::ConditionalBranch {
                condition,
                then_arguments,
                else_arguments,
                ..
            } => {
                terminator_reads.push(condition.clone());
                terminator_reads.extend(then_arguments.iter().cloned());
                terminator_reads.extend(else_arguments.iter().cloned());
            }
            SsaTerminator::Failure { .. } => {}
        }
        for value in terminator_reads {
            if !defined.contains(&value) {
                read_before_write.insert(value);
            }
        }
        uses[index] = read_before_write;
        defs[index] = defined;
    }
    // Backward liveness to a fixpoint. The transfer function models
    // block-parameter rebinding: successor params are killed (rebound at
    // the edge) while the edge arguments are read in the source block.
    let mut live_in: Vec<BTreeSet<SemanticId>> = vec![BTreeSet::new(); block_count];
    loop {
        let mut changed = false;
        for index in (0..block_count).rev() {
            let mut live_out = BTreeSet::new();
            for (target, arguments) in &successors[index] {
                let params: BTreeSet<&SemanticId> = function.blocks[*target]
                    .parameters
                    .iter()
                    .map(|parameter| &parameter.identity)
                    .collect();
                for value in &live_in[*target] {
                    if !params.contains(value) {
                        live_out.insert(value.clone());
                    }
                }
                for argument in arguments {
                    live_out.insert(argument.clone());
                }
            }
            live_out.retain(|value| !defs[index].contains(value));
            live_out.extend(uses[index].iter().cloned());
            if live_out != live_in[index] {
                live_in[index] = live_out;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    // Backedges target an already-emitted block; group latches by header.
    let mut header_latches: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (index, edges) in successors.iter().enumerate() {
        for (target, _) in edges {
            if *target <= index {
                header_latches.entry(*target).or_default().push(index);
            }
        }
    }
    // Reverse CFG for natural-loop body computation.
    let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); block_count];
    for (index, edges) in successors.iter().enumerate() {
        for (target, _) in edges {
            predecessors[*target].push(index);
        }
    }
    // Dominators: dom(entry) = {entry}; dom[B] = {B} ∪ ⋂ dom[preds].
    // A planned header must dominate every latch: otherwise the
    // "backedge" is a forward join to an earlier-emitted block (match
    // epilogues, shared exits), not a loop, and resetting there would
    // free cells that are still live on the fallthrough path.
    let mut dominators: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); block_count];
    if block_count > 0 {
        dominators[0].insert(0);
        for dominator in dominators.iter_mut().skip(1) {
            *dominator = (0..block_count).collect();
        }
        loop {
            let mut changed = false;
            let snapshot = dominators.clone();
            for (index, dominator) in dominators.iter_mut().enumerate().skip(1) {
                let mut dom: Option<BTreeSet<usize>> = None;
                for predecessor in &predecessors[index] {
                    dom = Some(match dom {
                        None => snapshot[*predecessor].clone(),
                        Some(intersection) => intersection
                            .intersection(&snapshot[*predecessor])
                            .copied()
                            .collect(),
                    });
                }
                let mut dom = dom.unwrap_or_default();
                dom.insert(index);
                if dom != *dominator {
                    *dominator = dom;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }
    for (header, latches) in &header_latches {
        let header = *header;
        // The header must dominate every latch; otherwise this is not a
        // natural loop at all.
        if latches
            .iter()
            .any(|latch| !dominators[*latch].contains(&header))
        {
            continue;
        }
        // Natural-loop body: the header plus every block that can reach a
        // latch without passing through the header.
        let mut body = vec![false; block_count];
        body[header] = true;
        let mut stack: Vec<usize> = latches.clone();
        while let Some(node) = stack.pop() {
            if body[node] {
                continue;
            }
            body[node] = true;
            stack.extend(predecessors[node].iter().copied());
        }
        // Single-entry check: every non-header body block's predecessors
        // must be inside the body. Without it, control could enter
        // mid-body with a cleared flag and reach a backedge before the
        // header re-pushes the mark, resetting to a stale mark.
        let mut single_entry = true;
        for (node, inside) in body.iter().enumerate() {
            if *inside && node != header {
                for predecessor in &predecessors[node] {
                    if !body[*predecessor] {
                        single_entry = false;
                        break;
                    }
                }
            }
            if !single_entry {
                break;
            }
        }
        if !single_entry {
            continue;
        }
        // The copy set preserves exactly the post-move locals that
        // downstream reads can observe: every cell-typed header parameter
        // (the argument move just rebound them to the new cells) plus
        // every other cell-typed value live at the header entry. Latch
        // argument values need no copying: dominance guarantees they are
        // rewritten in the latch before any post-reset read.
        let mut live = live_in[header].clone();
        for parameter in &function.blocks[header].parameters {
            live.insert(parameter.identity.clone());
        }
        let mut copies = Vec::new();
        let mut skip = false;
        for value in &live {
            let Some(ty) = tys.get(value) else {
                skip = true;
                break;
            };
            match copy_class_of_ty(ty, program, refmap, composites) {
                Ok(CopyClass::Cell(shape)) => {
                    copies.push((local(layout, value)?, shape));
                }
                Ok(CopyClass::Word) => {}
                Ok(CopyClass::View) | Err(_) => {
                    skip = true;
                    break;
                }
            }
        }
        if skip {
            continue;
        }
        copies.sort_by_key(|(slot, _)| *slot);
        copies.dedup_by_key(|(slot, _)| *slot);
        let words = copies
            .iter()
            .map(|(_, shape)| flatten_words(shape))
            .fold(0u32, |acc, words| acc.saturating_add(words));
        if words > REGION_TEMP_CAP_WORDS {
            continue;
        }
        let depth = copies
            .iter()
            .map(|(_, shape)| shape_depth(shape).saturating_add(1))
            .max()
            .unwrap_or(0);
        plan.addr_pool = plan.addr_pool.max(depth);
        plan.data_pool = plan.data_pool.max(words);
        plan.headers.insert(
            header as u32,
            HeaderPlan {
                mark_global: 0,
                active_global: 0,
                copies,
            },
        );
        // Exit edges leave the body: taking one deactivates the region so
        // the next entry re-pushes the mark. Backedges target the header
        // itself and are never exits.
        for (source, inside) in body.iter().enumerate() {
            if !inside {
                continue;
            }
            for (target, _) in &successors[source] {
                if !body[*target] {
                    plan.exits
                        .entry(source as u32)
                        .or_default()
                        .push((*target as u32, header as u32));
                }
            }
        }
    }
    // Exit entries name their region by header index; the caller rewrites
    // them to activation globals when it assigns mark/active indices.
    Ok(plan)
}

/// Record one CFG edge for region planning: resolve the target block and
/// append the successor with its argument bindings. Reads are accounted
/// separately by the order-sensitive use pass.
fn plan_edge(
    layout: &FunctionLayout,
    block_count: usize,
    successors: &mut [Vec<(usize, Vec<SemanticId>)>],
    index: usize,
    target: &SemanticId,
    arguments: &[SemanticId],
) -> Result<(), String> {
    let target_index = *layout
        .blocks
        .get(target)
        .ok_or_else(|| "branch target is missing from the function layout".to_owned())?
        as usize;
    if target_index >= block_count {
        return Err("branch target is outside the function blocks".to_owned());
    }
    successors[index].push((target_index, arguments.to_vec()));
    Ok(())
}

/// Emit one backedge's region sequence after its argument move: flatten
/// every live cell aside, reset the bump to the header mark (discarding
/// exactly this iteration's garbage), and rebuild the cells in the
/// parent region. Guarded on the host-buffer end cursor when one exists:
/// a host reservation inside the loop means the mark no longer bounds
/// scratch, so that iteration keeps status-quo behavior (locals stay
/// valid, nothing is freed).
fn emit_backedge_region(
    body: &mut Vec<Instr>,
    emit: &RegionEmit<'_>,
    header: &HeaderPlan,
) -> Result<(), String> {
    let mut sequence = Vec::new();
    let mut temp = emit.data_base;
    for (slot, shape) in &header.copies {
        emit_flatten(&mut sequence, *slot, shape, temp);
        temp = temp.saturating_add(flatten_words(shape));
    }
    sequence.push(Instr::GlobalGet(header.mark_global));
    sequence.push(Instr::GlobalSet(0));
    let mut temp = emit.data_base;
    for (slot, shape) in &header.copies {
        emit_unflatten(&mut sequence, emit.addr_base, shape, 0, temp, *slot)?;
        temp = temp.saturating_add(flatten_words(shape));
    }
    match emit.host_end {
        Some(host_end) => {
            body.push(Instr::GlobalGet(header.mark_global));
            body.push(Instr::GlobalGet(host_end));
            body.push(Instr::I32GeU);
            body.push(Instr::If);
            body.extend(sequence);
            body.push(Instr::End);
        }
        None => body.extend(sequence),
    }
    Ok(())
}

pub fn lower_module(
    program: &mncs_model::Program,
    ssa: &SsaModule,
    names: &[String],
) -> LoweringOutcome {
    let composites = CompositeInfo::from_program(program);
    let refmap = NominalRefMap::from_program(program, &composites);
    let mut functions = Vec::new();
    let mut exports = Vec::new();
    let mut unsupported = Vec::new();
    // Trigonometry imports come first in index space (`0..imports.len()`),
    // so every defined-function index shifts by the import count. The set
    // is fixed up front by scanning the whole module: deterministic and
    // independent of lowering order.
    let trig_imports = trig_import_list(ssa);
    let import_shift = trig_imports.len() as u32;
    let function_indices = ssa
        .functions
        .iter()
        .enumerate()
        .map(|(index, function)| {
            (
                function.semantic_identity.clone(),
                import_shift + index as u32,
            )
        })
        .collect::<BTreeMap<_, _>>();
    // Region pre-pass: mark-global indices are module-wide and must be
    // fixed before any function body is emitted. A function whose layout
    // or plan fails reuses the same error in the main loop below, so the
    // pre-pass never changes which functions lower.
    let mut prepared = Vec::with_capacity(ssa.functions.len());
    for function in &ssa.functions {
        prepared.push(
            layout_function(function, &function_indices, &composites).and_then(|layout| {
                plan_loop_regions(program, function, &layout, &composites, &refmap)
                    .map(|plan| (layout, plan))
            }),
        );
    }
    // Marks take globals 1..=region_count, activation flags take the next
    // region_count. `emit_alloc_helpers` appends the host-buffer end
    // cursor after both, so its index is one past the last flag whenever
    // the module materializes composites (and no host reservation can
    // exist otherwise, leaving backedge resets unguarded).
    let mut region_count = 0u32;
    for prepared_one in prepared.iter_mut().flatten() {
        for header in prepared_one.1.headers.values_mut() {
            region_count += 1;
            header.mark_global = region_count;
            header.active_global = region_count;
        }
    }
    for prepared_one in prepared.iter_mut().flatten() {
        let plan = &mut prepared_one.1;
        for header in plan.headers.values_mut() {
            header.active_global += region_count;
        }
        for edges in plan.exits.values_mut() {
            for (_, header_index) in edges.iter_mut() {
                // Exits only ever name headers planned in the same pass;
                // anything else is an internal defect, never a silent
                // global-0 clear.
                let active = plan
                    .headers
                    .get(&*header_index)
                    .expect("region exit references a planned header")
                    .active_global;
                *header_index = active;
            }
        }
    }
    let host_end = composites.uses_composites.then_some(1 + 2 * region_count);
    for (index, function) in ssa.functions.iter().enumerate() {
        let name = names
            .get(index)
            .cloned()
            .unwrap_or_else(|| export_name(&function.semantic_identity));
        let lowered = match &prepared[index] {
            Ok((layout, plan)) => lower_function(
                function,
                name,
                &trig_imports,
                &composites,
                layout,
                plan,
                host_end,
            ),
            Err(reason) => Err(reason.clone()),
        };
        match lowered {
            Ok(wasm) => {
                exports.push(wasm.name.clone());
                functions.push(wasm);
            }
            Err(reason) => unsupported.push(format!("{}: {reason}", function.identity.0)),
        }
    }
    let imports = trig_imports
        .iter()
        .map(|name| WasmImport {
            module: "mncs".to_owned(),
            name: name.clone(),
            params: vec![ValType::F64],
            results: vec![ValType::F64],
        })
        .collect::<Vec<_>>();
    let module = if unsupported.is_empty() && !functions.is_empty() {
        Some(if composites.uses_composites {
            // Global 0 is the bump cursor; the next region_count globals
            // are loop-region marks and the region_count after those are
            // the matching activation flags (index 0 is never either).
            // `emit_alloc_helpers` appends the host-buffer end cursor
            // after them.
            let mut globals = vec![crate::wasm::WasmGlobal {
                valtype: ValType::I32,
                mutable: true,
                init: 8,
            }];
            globals.extend((0..2 * region_count).map(|_| crate::wasm::WasmGlobal {
                valtype: ValType::I32,
                mutable: true,
                init: 0,
            }));
            WasmModule {
                imports: imports.clone(),
                globals,
                // Composite values are immutable cells. A bounded streaming
                // consumer may retain one state cell per input step, so the
                // old 1 MiB arena was too small for realistic byte streams
                // even though every individual sequence bound was finite.
                // Keep the artifact bounded while giving host-buffer-based
                // applications a documented 32 MiB arena budget.
                memory: Some(crate::wasm::WasmMemory { min_pages: 512 }),
                functions,
            }
        } else {
            WasmModule {
                imports,
                globals: Vec::new(),
                memory: None,
                functions,
            }
        })
    } else {
        None
    };
    LoweringOutcome {
        module,
        exports,
        unsupported,
    }
}

/// Sorted trigonometry imports used anywhere in the module (Profile
/// 0.12). Fixed up front so import indices are deterministic and
/// independent of lowering order; defined-function indices shift by the
/// import count.
fn trig_import_list(ssa: &SsaModule) -> Vec<String> {
    let mut names = BTreeSet::new();
    for function in &ssa.functions {
        for block in &function.blocks {
            for instruction in &block.instructions {
                if let SsaInstructionKind::FloatIntrinsic { function } = &instruction.kind {
                    names.insert(function.clone());
                }
            }
        }
    }
    names.into_iter().collect()
}

/// Per-type realization facts derived from the language-owned program.
#[derive(Debug, Clone, Default)]
struct CompositeInfo {
    /// record type identity -> canonical (field name, slot width)
    records: BTreeMap<SemanticId, Vec<(String, SlotWidth)>>,
    /// payload-bearing finite type identity -> discriminant -> payload fields
    boxed_finites: BTreeMap<SemanticId, BTreeMap<u32, Vec<(String, SlotWidth)>>>,
    uses_composites: bool,
}

impl CompositeInfo {
    fn from_program(program: &mncs_model::Program) -> Self {
        // Records and boxed finite variants occupy referenced cells; bare
        // tags and narrow scalars stay in 32-bit slots.
        let width_of = |semantic_type: &str| -> SlotWidth {
            // Canonical cells reserve eight bytes per field, but a nested
            // record/sequence/vector value is an i32 cell reference in the
            // portable WASM locals. The store/load width below is therefore
            // W32; the remaining slot bytes stay padding.
            if program
                .record_types
                .iter()
                .any(|record| record.name == semantic_type)
            {
                return SlotWidth::W32;
            }
            if program
                .finite_types
                .iter()
                .any(|finite| finite.name == semantic_type)
            {
                return SlotWidth::W32;
            }
            match mncs_model::BodyType::from_semantic_name(semantic_type) {
                mncs_model::BodyType::Integer(ty) if ty.bits == 64 => SlotWidth::W64,
                // Binary64 fields occupy a full 8-byte slot bit-carried as
                // u64 (I64Store/I64Load are bitwise); without this arm f64
                // fields collapsed to W32 and projections read back zeros
                // (ENG-PRESSURE-0002 WASM record divergence).
                mncs_model::BodyType::Float(ty) if ty.is_supported() => SlotWidth::W64,
                // Packed view descriptors and masks are full i64 values.
                // Exact sequences and vectors are i32 cell references, like
                // nested records above.
                mncs_model::BodyType::Sequence {
                    bound: mncs_model::SequenceBound::UpTo(_),
                    ..
                }
                | mncs_model::BodyType::Mask { .. } => SlotWidth::W64,
                _ => SlotWidth::W32,
            }
        };
        let mut info = Self::default();
        for record in &program.record_types {
            let fields = record
                .fields
                .iter()
                .map(|field| (field.name.clone(), width_of(&field.field_type)))
                .collect::<Vec<_>>();
            info.records.insert(record.identity.clone(), fields);
        }
        for finite in &program.finite_types {
            // A type is boxed when ANY variant carries a payload; every
            // variant of that type gets a layout entry (possibly empty).
            let boxed = finite
                .variants
                .iter()
                .any(|variant| !variant.payload.is_empty());
            if boxed {
                let variants = finite
                    .variants
                    .iter()
                    .map(|variant| {
                        (
                            variant.discriminant,
                            variant
                                .payload
                                .iter()
                                .map(|field| (field.name.clone(), width_of(&field.field_type)))
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect::<BTreeMap<_, _>>();
                info.boxed_finites.insert(finite.identity.clone(), variants);
            }
        }
        info.uses_composites = !info.records.is_empty()
            || !info.boxed_finites.is_empty()
            || program_uses_sequences(program);
        info
    }

    fn is_boxed_finite(&self, type_identity: &SemanticId) -> bool {
        self.boxed_finites.contains_key(type_identity)
    }
}

/// Whether any executable body materializes bounded sequences; such programs
/// need linear-memory arena support even without records or payload sums.
fn program_uses_sequences(program: &mncs_model::Program) -> bool {
    let ty_uses_arena =
        |ty: &BodyType| matches!(ty, BodyType::Sequence { .. } | BodyType::Vector { .. });
    let mut found = false;
    for function in &program.functions {
        if let Some(body) = &function.body {
            found |= body.parameters.iter().any(|param| ty_uses_arena(&param.ty));
            for block in &body.blocks {
                for operation in &block.operations {
                    found |= matches!(
                        operation.kind,
                        mncs_model::BodyOperationKind::SequenceConstruct { .. }
                            | mncs_model::BodyOperationKind::VectorConstruct { .. }
                            | mncs_model::BodyOperationKind::VectorSplat { .. }
                            | mncs_model::BodyOperationKind::VectorReplace { .. }
                            | mncs_model::BodyOperationKind::VectorBinary { .. }
                    );
                    found |= operation
                        .results
                        .iter()
                        .any(|result| ty_uses_arena(&result.ty));
                }
            }
        }
    }
    found
}

fn lower_function(
    function: &SsaFunction,
    name: String,
    trig_imports: &[String],
    composites: &CompositeInfo,
    layout: &FunctionLayout,
    plan: &RegionPlan,
    host_end: Option<u32>,
) -> Result<WasmFunction, String> {
    if function.outputs.len() != 1 {
        return Err("portable WASM MVP requires exactly one result".to_owned());
    }
    let result_ty = *layout
        .types
        .get(&function.outputs[0].identity)
        .ok_or_else(|| "function result type is unsupported".to_owned())?;
    let params = function
        .inputs
        .iter()
        .map(|input| {
            layout
                .types
                .get(&input.identity)
                .copied()
                .ok_or_else(|| "function parameter type is unsupported".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let local_count = layout.pc as usize + 1;
    let mut locals = vec![ValType::I32; local_count.saturating_sub(params.len())];
    for (identity, index) in &layout.values {
        if *index >= params.len() as u32 {
            locals[*index as usize - params.len()] = layout.types[identity];
        }
    }
    if layout.pc as usize >= params.len() {
        locals[layout.pc as usize - params.len()] = ValType::I32;
    }
    // Region scratch pools: I32 address temps then I64 data temps, shared
    // by every backedge sequence in the function.
    let addr_base = local_count as u32;
    locals.extend(std::iter::repeat_n(ValType::I32, plan.addr_pool as usize));
    let data_base = addr_base + plan.addr_pool;
    locals.extend(std::iter::repeat_n(ValType::I64, plan.data_pool as usize));
    let regions = RegionEmit {
        plan,
        addr_base,
        data_base,
        host_end,
    };
    let mut body = Vec::new();
    // Deactivate every region at function entry: a previous call may have
    // returned from inside a loop (leaving its flag set), and recursion
    // re-enters with stale marks. Clearing first makes each activation
    // push a fresh mark at its first header visit.
    let mut entry_regions: Vec<&HeaderPlan> = plan.headers.values().collect();
    entry_regions.sort_by_key(|header| header.active_global);
    for header in entry_regions {
        body.push(Instr::I32Const(0));
        body.push(Instr::GlobalSet(header.active_global));
    }
    body.push(Instr::I32Const(0));
    body.push(Instr::LocalSet(layout.pc));
    body.push(Instr::Loop);
    for (block_index, block) in function.blocks.iter().enumerate() {
        body.push(Instr::LocalGet(layout.pc));
        body.push(Instr::I32Const(block_index as i32));
        body.push(Instr::I32Eq);
        body.push(Instr::If);
        // Loop-region activation: on the first header visit of an
        // activation, record the bump as the mark bounding the whole
        // activation's garbage. Backedges do not re-push (the flag stays
        // set), so each backedge reset frees the previous trip's garbage
        // AND the previous trip's rebuilt duplicates together.
        if let Some(header) = plan.headers.get(&(block_index as u32)) {
            body.push(Instr::GlobalGet(header.active_global));
            body.push(Instr::I32Eqz);
            body.push(Instr::If);
            body.push(Instr::GlobalGet(0));
            body.push(Instr::GlobalSet(header.mark_global));
            body.push(Instr::I32Const(1));
            body.push(Instr::GlobalSet(header.active_global));
            body.push(Instr::End);
        }
        for instruction in &block.instructions {
            lower_instruction(layout, instruction, &mut body, trig_imports, composites)?;
        }
        lower_terminator(
            layout,
            &block.terminator,
            &mut body,
            &regions,
            block_index as u32,
        )?;
        body.push(Instr::End);
    }
    // Close the dispatcher loop before the terminal fallback. Keeping the
    // unreachable outside the loop makes the function-level fallthrough
    // unreachable to a standard WASM validator while preserving the
    // embedded interpreter's fail-closed behavior.
    body.push(Instr::End);
    body.push(Instr::Unreachable);
    Ok(WasmFunction {
        name,
        params,
        results: vec![result_ty],
        locals,
        body,
    })
}

fn layout_function(
    function: &SsaFunction,
    function_indices: &BTreeMap<SemanticId, u32>,
    _composites: &CompositeInfo,
) -> Result<FunctionLayout, String> {
    let mut values = BTreeMap::new();
    let mut types = BTreeMap::new();
    let mut seen = BTreeSet::new();
    let mut next = 0_u32;
    let mut push = |value: &SsaValue| -> Result<(), String> {
        if !seen.insert(value.identity.clone()) {
            return Ok(());
        }
        let (wasm, integer) = wasm_type(&value.ty)?;
        values.insert(value.identity.clone(), next);
        types.insert(value.identity.clone(), wasm);
        let _ = integer;
        next += 1;
        Ok(())
    };
    for input in &function.inputs {
        push(input)?;
    }
    for block in &function.blocks {
        for parameter in &block.parameters {
            push(parameter)?;
        }
        for instruction in &block.instructions {
            for output in &instruction.outputs {
                push(output)?;
            }
        }
    }
    for output in &function.outputs {
        push(output)?;
    }
    let pc = next;
    let blocks = function
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.identity.clone(), index as u32))
        .collect();
    let block_params = function
        .blocks
        .iter()
        .map(|block| {
            (
                block.identity.clone(),
                block
                    .parameters
                    .iter()
                    .map(|parameter| parameter.identity.clone())
                    .collect(),
            )
        })
        .collect();
    Ok(FunctionLayout {
        values,
        types,
        pc,
        blocks,
        block_params,
        functions: function_indices.clone(),
    })
}

fn lower_instruction(
    layout: &FunctionLayout,
    instruction: &mncs_model::SsaInstruction,
    body: &mut Vec<Instr>,
    trig_imports: &[String],
    composites: &CompositeInfo,
) -> Result<(), String> {
    match &instruction.kind {
        SsaInstructionKind::Constant { value, ty } => {
            let dest = dest_local(layout, instruction)?;
            emit_const(body, *value, ty)?;
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::Integer {
            operator,
            operand_type,
            intent,
        } => {
            let dest = dest_local(layout, instruction)?;
            let left = operand_local(layout, instruction, 0)?;
            let right = operand_local(layout, instruction, 1)?;
            let result_type = instruction
                .outputs
                .first()
                .and_then(|output| wasm_type(&output.ty).ok().and_then(|(_, integer)| integer))
                .ok_or_else(|| "integer instruction result has no integer type".to_owned())?;
            emit_integer(
                body,
                operator,
                *operand_type,
                result_type,
                *intent,
                [left, right, dest],
            )?;
        }
        SsaInstructionKind::IntegerCompare {
            predicate,
            operand_type,
        } => {
            let dest = dest_local(layout, instruction)?;
            let left = operand_local(layout, instruction, 0)?;
            let right = operand_local(layout, instruction, 1)?;
            body.push(Instr::LocalGet(left));
            body.push(Instr::LocalGet(right));
            body.push(compare_instr(predicate, *operand_type)?);
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::FiniteConstruct {
            type_identity,
            discriminant,
            payload_fields,
            ..
        } => {
            let dest = dest_local(layout, instruction)?;
            match composites.boxed_finites.get(type_identity) {
                Some(variants) => {
                    // Boxed representation: allocate a tag cell plus one
                    // 8-byte slot per payload field of THIS variant.
                    let fields = variants.get(discriminant).ok_or_else(|| {
                        format!("finite variant {discriminant} has no declared payload layout")
                    })?;
                    if fields.len() != payload_fields.len() {
                        return Err(
                            "payload field count does not match the declared variant layout"
                                .to_owned(),
                        );
                    }
                    emit_alloc(body, dest, (fields.len() as u32 + 1) * 8)?;
                    body.push(Instr::LocalGet(dest));
                    body.push(Instr::I32Const(*discriminant as i32));
                    body.push(Instr::I32Store);
                    for (index, _) in fields.iter().enumerate() {
                        let operand = operand_local(layout, instruction, index)?;
                        let offset = ((index + 1) * 8) as i32;
                        body.push(Instr::LocalGet(dest));
                        emit_offset(body, offset);
                        body.push(Instr::LocalGet(operand));
                        store_instr(layout, instruction, index, body)?;
                    }
                }
                None => {
                    if !payload_fields.is_empty() {
                        return Err(
                            "payload-bearing construction on an unboxed finite type".to_owned()
                        );
                    }
                    body.push(Instr::I32Const(*discriminant as i32));
                    body.push(Instr::LocalSet(dest));
                }
            }
        }
        SsaInstructionKind::BooleanOp { operator } => {
            // Bools are normalized 0/1 i32 locals in this realization, so
            // strict conjunction/disjunction is a bitwise op on them.
            let dest = dest_local(layout, instruction)?;
            let left = operand_local(layout, instruction, 0)?;
            let right = operand_local(layout, instruction, 1)?;
            body.push(Instr::LocalGet(left));
            body.push(Instr::LocalGet(right));
            body.push(match operator.as_str() {
                "and" => Instr::I32And,
                "or" => Instr::I32Or,
                other => return Err(format!("unsupported boolean operator {other}")),
            });
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::BooleanCompare { predicate } => {
            // Normalized 0/1 i32 locals compare exactly with integer
            // equality; only `eq`/`ne` exist on bools by construction.
            let dest = dest_local(layout, instruction)?;
            let left = operand_local(layout, instruction, 0)?;
            let right = operand_local(layout, instruction, 1)?;
            body.push(Instr::LocalGet(left));
            body.push(Instr::LocalGet(right));
            body.push(match predicate {
                p if p == "eq" => Instr::I32Eq,
                p if p == "ne" => Instr::I32Ne,
                other => return Err(format!("unsupported boolean comparison predicate {other}")),
            });
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::BooleanNot => {
            // Normalized 0/1 i32 local: logical not is test-against-zero.
            let dest = dest_local(layout, instruction)?;
            let src = operand_local(layout, instruction, 0)?;
            body.push(Instr::LocalGet(src));
            body.push(Instr::I32Eqz);
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::ByteBitwise { operator } => {
            // Bytes ride zero-extended in i32 cells; bitwise ops are exact.
            let dest = dest_local(layout, instruction)?;
            let left = operand_local(layout, instruction, 0)?;
            let right = operand_local(layout, instruction, 1)?;
            body.push(Instr::LocalGet(left));
            body.push(Instr::LocalGet(right));
            body.push(match operator.as_str() {
                "and" => Instr::I32And,
                "or" => Instr::I32Or,
                "xor" => Instr::I32Xor,
                other => return Err(format!("unsupported byte bitwise operator {other}")),
            });
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::FloatConstant { bits, ty } => {
            if !ty.is_supported() {
                return Err("only binary64 float constants are supported".to_owned());
            }
            let dest = dest_local(layout, instruction)?;
            body.push(Instr::F64Const(*bits));
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::Float { operator } => {
            let instruction_op = match operator.as_str() {
                "add" => Instr::F64Add,
                "sub" => Instr::F64Sub,
                "mul" => Instr::F64Mul,
                "div" => Instr::F64Div,
                _ => {
                    return Err(format!("unsupported float operator {operator}"));
                }
            };
            let dest = dest_local(layout, instruction)?;
            let left = operand_local(layout, instruction, 0)?;
            let right = operand_local(layout, instruction, 1)?;
            // The non-finite trap rule, realized as the conservative
            // fallback: guard both inputs and the result.
            emit_finite_guard(body, left);
            emit_finite_guard(body, right);
            body.push(Instr::LocalGet(left));
            body.push(Instr::LocalGet(right));
            body.push(instruction_op);
            body.push(Instr::LocalSet(dest));
            emit_finite_guard(body, dest);
        }
        SsaInstructionKind::FloatIntrinsic { function } => {
            if !matches!(function.as_str(), "sin" | "cos") {
                return Err(format!("unsupported float intrinsic {function}"));
            }
            let dest = dest_local(layout, instruction)?;
            let src = operand_local(layout, instruction, 0)?;
            // The non-finite trap rule, realized as the conservative
            // fallback: guard the input and the result around the host
            // call, exactly like arithmetic.
            emit_finite_guard(body, src);
            body.push(Instr::LocalGet(src));
            let import = trig_imports
                .iter()
                .position(|import| import == function)
                .ok_or_else(|| format!("float intrinsic {function} was not imported"))?;
            body.push(Instr::Call(import as u32));
            body.push(Instr::LocalSet(dest));
            emit_finite_guard(body, dest);
        }
        SsaInstructionKind::FloatCompare { predicate } => {
            let instruction_op = match predicate.as_str() {
                "eq" => Instr::F64Eq,
                "ne" => Instr::F64Ne,
                "lt" => Instr::F64Lt,
                "le" => Instr::F64Le,
                "gt" => Instr::F64Gt,
                "ge" => Instr::F64Ge,
                _ => {
                    return Err(format!(
                        "unsupported float comparison predicate {predicate}"
                    ));
                }
            };
            let dest = dest_local(layout, instruction)?;
            let left = operand_local(layout, instruction, 0)?;
            let right = operand_local(layout, instruction, 1)?;
            emit_finite_guard(body, left);
            emit_finite_guard(body, right);
            body.push(Instr::LocalGet(left));
            body.push(Instr::LocalGet(right));
            body.push(instruction_op);
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::ByteShift { operator } => {
            // Total semantics: the u64 count is taken modulo 8 (count rides
            // i64), shifts are logical, and the result is masked to a byte.
            let dest = dest_local(layout, instruction)?;
            let left = operand_local(layout, instruction, 0)?;
            let right = operand_local(layout, instruction, 1)?;
            body.push(Instr::LocalGet(left));
            body.push(Instr::LocalGet(right));
            body.push(Instr::I64Const(8));
            body.push(Instr::I64RemU);
            body.push(Instr::I32WrapI64);
            if operator == "shl" {
                body.push(Instr::I32Shl);
                body.push(Instr::I32Const(255));
                body.push(Instr::I32And);
            } else if operator == "shr" {
                body.push(Instr::I32ShrU);
            } else {
                return Err(format!("unsupported byte shift operator {operator}"));
            }
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::ByteCompare { predicate } => {
            // Bytes compare as unsigned 8-bit values in i32 cells.
            let dest = dest_local(layout, instruction)?;
            let left = operand_local(layout, instruction, 0)?;
            let right = operand_local(layout, instruction, 1)?;
            body.push(Instr::LocalGet(left));
            body.push(Instr::LocalGet(right));
            body.push(compare_instr(
                predicate,
                IntegerType {
                    bits: 8,
                    signed: false,
                },
            )?);
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::Convert { from, to } => {
            emit_convert(layout, instruction, body, from.clone(), to.clone())?
        }
        SsaInstructionKind::SequenceConstruct {
            element_type: _,
            length,
        } => {
            // Exact sequences materialize as canonical cells with one
            // 8-byte slot per element, exactly like record layouts.
            let dest = dest_local(layout, instruction)?;
            emit_alloc(body, dest, *length * 8)?;
            for index in 0..*length as usize {
                let operand = operand_local(layout, instruction, index)?;
                body.push(Instr::LocalGet(dest));
                emit_offset(body, (index * 8) as i32);
                body.push(Instr::LocalGet(operand));
                store_element_width(layout, instruction, index, body)?;
            }
        }
        SsaInstructionKind::Select { operand_type } => {
            let dest = dest_local(layout, instruction)?;
            let condition = operand_local(layout, instruction, 0)?;
            let when_true = operand_local(layout, instruction, 1)?;
            let when_false = operand_local(layout, instruction, 2)?;
            if let BodyType::Vector { element, lanes } = operand_type {
                let (lane_type, lane_bytes, integer) = vector_lane_realization(element)?;
                emit_alloc(
                    body,
                    dest,
                    lanes
                        .checked_mul(lane_bytes)
                        .ok_or_else(|| "vector allocation size overflow".to_owned())?,
                )?;
                for lane in 0..*lanes {
                    let offset = lane
                        .checked_mul(lane_bytes)
                        .ok_or_else(|| "vector lane offset overflow".to_owned())?
                        as i32;
                    body.push(Instr::LocalGet(dest));
                    emit_offset(body, offset);
                    body.push(Instr::LocalGet(when_true));
                    emit_offset(body, offset);
                    load_valtype(lane_type, body);
                    body.push(Instr::LocalGet(when_false));
                    emit_offset(body, offset);
                    load_valtype(lane_type, body);
                    body.push(Instr::LocalGet(condition));
                    body.push(Instr::I64Const(i64::from(lane)));
                    body.push(Instr::I64ShrU);
                    body.push(Instr::I32WrapI64);
                    body.push(Instr::I32Const(1));
                    body.push(Instr::I32And);
                    body.push(Instr::Select);
                    store_valtype(lane_type, body);
                }
                let _ = integer;
                return Ok(());
            }
            // WebAssembly `select` consumes true, false, condition and does
            // not introduce a candidate-dependent control-flow edge.
            body.push(Instr::LocalGet(when_true));
            body.push(Instr::LocalGet(when_false));
            body.push(Instr::LocalGet(condition));
            body.push(Instr::Select);
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::SequenceReplace {
            bound, evidence, ..
        } => {
            let mncs_model::SequenceBound::Exact(length) = bound else {
                return Err("functional sequence update requires an exact bound".to_owned());
            };
            let dest = dest_local(layout, instruction)?;
            let source = operand_local(layout, instruction, 0)?;
            let index = operand_local(layout, instruction, 1)?;
            let element = operand_local(layout, instruction, 2)?;
            if matches!(evidence, mncs_model::BoundsEvidence::RuntimeChecked { .. }) {
                body.push(Instr::LocalGet(index));
                body.push(Instr::I64Const(i64::from(*length)));
                body.push(Instr::I64GeU);
                body.push(Instr::If);
                body.push(Instr::Unreachable);
                body.push(Instr::End);
            }
            // Allocate a fresh canonical cell and copy every fixed slot. The
            // source is immutable, so replacement preserves value semantics.
            emit_alloc(body, dest, *length * 8)?;
            let element_valtype = instruction
                .inputs
                .get(2)
                .and_then(|identity| layout.types.get(identity))
                .copied()
                .unwrap_or(ValType::I32);
            for lane in 0..*length {
                body.push(Instr::LocalGet(dest));
                emit_offset(body, (lane * 8) as i32);
                body.push(Instr::LocalGet(source));
                emit_offset(body, (lane * 8) as i32);
                match element_valtype {
                    ValType::I32 => body.push(Instr::I32Load),
                    ValType::I64 => body.push(Instr::I64Load),
                    // Float sequences stay refused in C1: element lowering
                    // is per-width on every backend.
                    ValType::F64 => {
                        return Err("float sequences are not supported".to_owned());
                    }
                }
                store_element_width(layout, instruction, 2, body)?;
            }
            body.push(Instr::LocalGet(dest));
            body.push(Instr::LocalGet(index));
            body.push(Instr::I64Const(3));
            body.push(Instr::I64Shl);
            body.push(Instr::I32WrapI64);
            body.push(Instr::I32Add);
            body.push(Instr::LocalGet(element));
            store_element_width(layout, instruction, 2, body)?;
        }
        SsaInstructionKind::VectorConstruct {
            element_type,
            lanes,
        } => {
            let dest = dest_local(layout, instruction)?;
            let (lane_type, lane_bytes, _) = vector_lane_realization(element_type)?;
            if instruction.inputs.len() != *lanes as usize {
                return Err(
                    "vector construction operand count does not match lane count".to_owned(),
                );
            }
            emit_alloc(
                body,
                dest,
                lanes
                    .checked_mul(lane_bytes)
                    .ok_or_else(|| "vector allocation size overflow".to_owned())?,
            )?;
            for lane in 0..*lanes {
                body.push(Instr::LocalGet(dest));
                emit_offset(
                    body,
                    lane.checked_mul(lane_bytes)
                        .ok_or_else(|| "vector lane offset overflow".to_owned())?
                        as i32,
                );
                body.push(Instr::LocalGet(operand_local(
                    layout,
                    instruction,
                    lane as usize,
                )?));
                store_valtype(lane_type, body);
            }
        }
        SsaInstructionKind::VectorSplat {
            element_type,
            lanes,
        } => {
            let dest = dest_local(layout, instruction)?;
            let value = operand_local(layout, instruction, 0)?;
            let (lane_type, lane_bytes, _) = vector_lane_realization(element_type)?;
            emit_alloc(
                body,
                dest,
                lanes
                    .checked_mul(lane_bytes)
                    .ok_or_else(|| "vector allocation size overflow".to_owned())?,
            )?;
            for lane in 0..*lanes {
                body.push(Instr::LocalGet(dest));
                emit_offset(
                    body,
                    lane.checked_mul(lane_bytes)
                        .ok_or_else(|| "vector lane offset overflow".to_owned())?
                        as i32,
                );
                body.push(Instr::LocalGet(value));
                store_valtype(lane_type, body);
            }
        }
        SsaInstructionKind::VectorExtract {
            element_type,
            lanes,
            evidence,
        } => {
            let dest = dest_local(layout, instruction)?;
            let vector = operand_local(layout, instruction, 0)?;
            let index = operand_local(layout, instruction, 1)?;
            let (lane_type, lane_bytes, _) = vector_lane_realization(element_type)?;
            emit_vector_lane_check(body, index, *lanes, evidence);
            body.push(Instr::LocalGet(vector));
            body.push(Instr::LocalGet(index));
            body.push(Instr::I64Const(i64::from(lane_bytes)));
            body.push(Instr::I64Mul);
            body.push(Instr::I32WrapI64);
            body.push(Instr::I32Add);
            load_valtype(lane_type, body);
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::VectorReplace {
            element_type,
            lanes,
            evidence,
        } => {
            let dest = dest_local(layout, instruction)?;
            let source = operand_local(layout, instruction, 0)?;
            let index = operand_local(layout, instruction, 1)?;
            let element = operand_local(layout, instruction, 2)?;
            let (lane_type, lane_bytes, _) = vector_lane_realization(element_type)?;
            emit_vector_lane_check(body, index, *lanes, evidence);
            emit_alloc(
                body,
                dest,
                lanes
                    .checked_mul(lane_bytes)
                    .ok_or_else(|| "vector allocation size overflow".to_owned())?,
            )?;
            for lane in 0..*lanes {
                let offset = lane
                    .checked_mul(lane_bytes)
                    .ok_or_else(|| "vector lane offset overflow".to_owned())?
                    as i32;
                body.push(Instr::LocalGet(dest));
                emit_offset(body, offset);
                body.push(Instr::LocalGet(source));
                emit_offset(body, offset);
                load_valtype(lane_type, body);
                store_valtype(lane_type, body);
            }
            body.push(Instr::LocalGet(dest));
            body.push(Instr::LocalGet(index));
            body.push(Instr::I64Const(i64::from(lane_bytes)));
            body.push(Instr::I64Mul);
            body.push(Instr::I32WrapI64);
            body.push(Instr::I32Add);
            body.push(Instr::LocalGet(element));
            store_valtype(lane_type, body);
        }
        SsaInstructionKind::VectorBinary {
            operator,
            element_type,
            lanes,
            intent,
        } => {
            if !matches!(intent, ArithmeticIntent::Wrapping) {
                return Err(format!("portable WASM scalar vector realization does not yet support {intent:?} lane arithmetic"));
            }
            let dest = dest_local(layout, instruction)?;
            let left = operand_local(layout, instruction, 0)?;
            let right = operand_local(layout, instruction, 1)?;
            let (lane_type, lane_bytes, integer) = vector_lane_realization(element_type)?;
            emit_alloc(
                body,
                dest,
                lanes
                    .checked_mul(lane_bytes)
                    .ok_or_else(|| "vector allocation size overflow".to_owned())?,
            )?;
            for lane in 0..*lanes {
                let offset = lane
                    .checked_mul(lane_bytes)
                    .ok_or_else(|| "vector lane offset overflow".to_owned())?
                    as i32;
                body.push(Instr::LocalGet(dest));
                emit_offset(body, offset);
                emit_vector_binary_lane(body, lane_type, integer, operator, left, right, offset)?;
                store_valtype(lane_type, body);
            }
        }
        SsaInstructionKind::VectorCompare {
            predicate,
            element_type,
            lanes,
        } => {
            let dest = dest_local(layout, instruction)?;
            let left = operand_local(layout, instruction, 0)?;
            let right = operand_local(layout, instruction, 1)?;
            let (lane_type, lane_bytes, integer) = vector_lane_realization(element_type)?;
            body.push(Instr::I64Const(0));
            body.push(Instr::LocalSet(dest));
            for lane in 0..*lanes {
                let offset = lane
                    .checked_mul(lane_bytes)
                    .ok_or_else(|| "vector lane offset overflow".to_owned())?
                    as i32;
                body.push(Instr::LocalGet(dest));
                body.push(Instr::LocalGet(left));
                emit_offset(body, offset);
                load_valtype(lane_type, body);
                body.push(Instr::LocalGet(right));
                emit_offset(body, offset);
                load_valtype(lane_type, body);
                body.push(compare_instr(predicate, integer)?);
                body.push(Instr::I64ExtendI32U);
                if lane != 0 {
                    body.push(Instr::I64Const(i64::from(lane)));
                    body.push(Instr::I64Shl);
                }
                body.push(Instr::I64Or);
                body.push(Instr::LocalSet(dest));
            }
        }
        SsaInstructionKind::MaskBinary { operator, lanes } => {
            let dest = dest_local(layout, instruction)?;
            body.push(Instr::LocalGet(operand_local(layout, instruction, 0)?));
            body.push(Instr::LocalGet(operand_local(layout, instruction, 1)?));
            body.push(match operator.as_str() {
                "and" => Instr::I64And,
                "or" => Instr::I64Or,
                "xor" => Instr::I64Xor,
                other => return Err(format!("unsupported mask operator {other}")),
            });
            emit_mask_limit(body, *lanes);
            body.push(Instr::I64And);
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::MaskNot { lanes } => {
            let dest = dest_local(layout, instruction)?;
            body.push(Instr::LocalGet(operand_local(layout, instruction, 0)?));
            emit_mask_limit(body, *lanes);
            body.push(Instr::I64Xor);
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::MaskReduce { operator, lanes } => {
            let dest = dest_local(layout, instruction)?;
            let value = operand_local(layout, instruction, 0)?;
            body.push(Instr::LocalGet(value));
            match operator.as_str() {
                "any" => body.push(Instr::I64Eqz),
                "none" => body.push(Instr::I64Eqz),
                "all" => {
                    emit_mask_limit(body, *lanes);
                    body.push(Instr::I64Eq);
                }
                other => return Err(format!("unsupported mask reduction {other}")),
            }
            if operator == "any" {
                body.push(Instr::I32Eqz);
            }
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::VectorReduce {
            operator,
            element_type,
            lanes,
            intent,
        } => {
            if *lanes == 0 {
                return Err("vector reduction requires at least one lane".to_owned());
            }
            if !matches!(intent, ArithmeticIntent::Wrapping) {
                return Err(format!(
                    "portable WASM scalar vector reduction does not yet support {intent:?}"
                ));
            }
            let dest = dest_local(layout, instruction)?;
            let vector = operand_local(layout, instruction, 0)?;
            let (lane_type, lane_bytes, integer) = vector_lane_realization(element_type)?;
            let seed = match operator.as_str() {
                "sum" => 0,
                "min" if integer.signed && integer.bits == 32 => i64::from(i32::MAX),
                "min" if integer.signed => i64::MAX,
                "min" if integer.bits == 32 => i64::from(u32::MAX),
                "min" => -1,
                "max" if integer.signed && integer.bits == 32 => i64::from(i32::MIN),
                "max" if integer.signed => i64::MIN,
                "max" => 0,
                other => return Err(format!("unsupported vector reduction {other}")),
            };
            body.push(match lane_type {
                ValType::I32 => Instr::I32Const(seed as i32),
                ValType::I64 => Instr::I64Const(seed),
                ValType::F64 => {
                    return Err("float vectors are not supported".to_owned());
                }
            });
            body.push(Instr::LocalSet(dest));
            for lane in 0..*lanes {
                emit_vector_reduce_lane(
                    body,
                    dest,
                    vector,
                    (lane * lane_bytes) as i32,
                    lane_type,
                    integer,
                    operator,
                )?;
            }
        }
        SsaInstructionKind::SequenceProject { bound, evidence } => {
            let dest = dest_local(layout, instruction)?;
            let seq = operand_local(layout, instruction, 0)?;
            let index = operand_local(layout, instruction, 1)?;
            let checked = matches!(evidence, mncs_model::BoundsEvidence::RuntimeChecked { .. });
            match bound {
                mncs_model::SequenceBound::Exact(length) => {
                    if checked {
                        // Out-of-bounds indexes trap exactly like checked
                        // arithmetic edges.
                        body.push(Instr::LocalGet(index));
                        body.push(Instr::I64Const(i64::from(*length)));
                        body.push(Instr::I64GeU);
                        body.push(Instr::If);
                        body.push(Instr::Unreachable);
                        body.push(Instr::End);
                    }
                    body.push(Instr::LocalGet(seq));
                    body.push(Instr::LocalGet(index));
                    body.push(Instr::I64Const(3));
                    body.push(Instr::I64Shl);
                    body.push(Instr::I32WrapI64);
                    body.push(Instr::I32Add);
                    load_element_width(layout, instruction, body)?;
                }
                mncs_model::SequenceBound::UpTo(_) => {
                    if checked {
                        body.push(Instr::LocalGet(index));
                        body.push(Instr::LocalGet(seq));
                        body.push(Instr::I64Const(32));
                        body.push(Instr::I64ShrU);
                        // Discard the internal cell marker (bit 63) so the
                        // bound check observes the true runtime length.
                        body.push(Instr::I64Const(0x7fff_ffff));
                        body.push(Instr::I64And);
                        body.push(Instr::I64GeU);
                        body.push(Instr::If);
                        body.push(Instr::Unreachable);
                        body.push(Instr::End);
                    }
                    if matches!(&instruction.outputs[0].ty, IrType::Named(name) if name == "byte") {
                        // Bit 63 marks a byte view whose source is still an
                        // exact canonical-cell sequence. Host-packed byte
                        // views leave it clear. The low 32 bits always carry
                        // the true element address (packed slices may start
                        // at an odd address, so no address bit may serve as
                        // the marker); preserve the source representation's
                        // stride for the index.
                        body.push(Instr::LocalGet(seq));
                        body.push(Instr::I32WrapI64);
                        body.push(Instr::LocalGet(index));
                        body.push(Instr::I64Const(3));
                        body.push(Instr::I64Shl);
                        body.push(Instr::LocalGet(index));
                        body.push(Instr::LocalGet(seq));
                        body.push(Instr::I64Const(63));
                        body.push(Instr::I64ShrU);
                        body.push(Instr::I32WrapI64);
                        body.push(Instr::Select);
                        body.push(Instr::I32WrapI64);
                    } else {
                        body.push(Instr::LocalGet(seq));
                        body.push(Instr::I32WrapI64);
                        body.push(Instr::LocalGet(index));
                        body.push(Instr::I64Const(3));
                        body.push(Instr::I64Shl);
                        body.push(Instr::I32WrapI64);
                    }
                    body.push(Instr::I32Add);
                    load_element_width(layout, instruction, body)?;
                }
                mncs_model::SequenceBound::Param(_) | mncs_model::SequenceBound::UpToParam(_) => {
                    unreachable!(
                        "generic SequenceBound must be specialized before backend lowering"
                    )
                }
            }
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::SequenceLength { bound } => {
            let dest = dest_local(layout, instruction)?;
            let seq = operand_local(layout, instruction, 0)?;
            match bound {
                mncs_model::SequenceBound::Exact(length) => {
                    body.push(Instr::I64Const(i64::from(*length)));
                }
                mncs_model::SequenceBound::UpTo(_) => {
                    body.push(Instr::LocalGet(seq));
                    body.push(Instr::I64Const(32));
                    body.push(Instr::I64ShrU);
                    // Discard the internal cell marker (bit 63); lengths
                    // never carry it.
                    body.push(Instr::I64Const(0x7fff_ffff));
                    body.push(Instr::I64And);
                }
                mncs_model::SequenceBound::Param(_) | mncs_model::SequenceBound::UpToParam(_) => {
                    unreachable!(
                        "generic SequenceBound must be specialized before backend lowering"
                    )
                }
            }
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::ViewConstruct {
            source_bound,
            view_bound,
        } => {
            let mncs_model::SequenceBound::UpTo(cap) = view_bound else {
                return Err("view construction must produce an UpTo view".to_owned());
            };
            let dest = dest_local(layout, instruction)?;
            let seq = operand_local(layout, instruction, 0)?;
            let start = operand_local(layout, instruction, 1)?;
            let end = operand_local(layout, instruction, 2)?;
            // Source length: static for exact sequences, packed for views.
            let source_len = |body: &mut Vec<Instr>| match source_bound {
                mncs_model::SequenceBound::Exact(length) => {
                    body.push(Instr::I64Const(i64::from(*length)));
                }
                mncs_model::SequenceBound::UpTo(_) => {
                    body.push(Instr::LocalGet(seq));
                    body.push(Instr::I64Const(32));
                    body.push(Instr::I64ShrU);
                    // Discard the internal cell marker (bit 63) so the
                    // end-bounds trap observes the true source length.
                    body.push(Instr::I64Const(0x7fff_ffff));
                    body.push(Instr::I64And);
                }
                mncs_model::SequenceBound::Param(_) | mncs_model::SequenceBound::UpToParam(_) => {
                    unreachable!(
                        "generic SequenceBound must be specialized before backend lowering"
                    )
                }
            };
            // start > end -> trap
            body.push(Instr::LocalGet(start));
            body.push(Instr::LocalGet(end));
            body.push(Instr::I64GtU);
            body.push(Instr::If);
            body.push(Instr::Unreachable);
            body.push(Instr::End);
            // end > len(source) -> trap
            body.push(Instr::LocalGet(end));
            source_len(body);
            body.push(Instr::I64GtU);
            body.push(Instr::If);
            body.push(Instr::Unreachable);
            body.push(Instr::End);
            // span > cap -> trap
            body.push(Instr::LocalGet(end));
            body.push(Instr::LocalGet(start));
            body.push(Instr::I64Sub);
            body.push(Instr::I64Const(i64::from(*cap)));
            body.push(Instr::I64GtU);
            body.push(Instr::If);
            body.push(Instr::Unreachable);
            body.push(Instr::End);
            // Exact sequences use canonical eight-byte cells; an UpTo source
            // already carries a packed view descriptor whose low 32 bits are
            // the true byte address of its element representation. Preserve
            // that representation when applying the slice start. The
            // cell-backed marker lives in bit 63 (see
            // crate::composite::VIEW_CELL_MARKER): no address bit may serve
            // as the marker because packed slices may start at odd offsets.
            let byte_view = matches!(&instruction.outputs[0].ty, IrType::Named(name) if name.contains("[byte;"));
            match source_bound {
                mncs_model::SequenceBound::Exact(_) => {
                    body.push(Instr::LocalGet(seq));
                    body.push(Instr::I64ExtendI32U);
                    body.push(Instr::LocalGet(start));
                    body.push(Instr::I64Const(3));
                    body.push(Instr::I64Shl);
                }
                mncs_model::SequenceBound::UpTo(_) => {
                    body.push(Instr::LocalGet(seq));
                    body.push(Instr::I64Const(4_294_967_295));
                    body.push(Instr::I64And);
                    if byte_view {
                        // Select start*8 for an exact-cell byte view (marker
                        // set) and start for a packed host byte view.
                        body.push(Instr::LocalGet(start));
                        body.push(Instr::I64Const(3));
                        body.push(Instr::I64Shl);
                        body.push(Instr::LocalGet(start));
                        body.push(Instr::LocalGet(seq));
                        body.push(Instr::I64Const(63));
                        body.push(Instr::I64ShrU);
                        body.push(Instr::I32WrapI64);
                        body.push(Instr::Select);
                    } else {
                        body.push(Instr::LocalGet(start));
                        body.push(Instr::I64Const(3));
                        body.push(Instr::I64Shl);
                    }
                }
                mncs_model::SequenceBound::Param(_) | mncs_model::SequenceBound::UpToParam(_) => {
                    unreachable!(
                        "generic SequenceBound must be specialized before backend lowering"
                    )
                }
            }
            body.push(Instr::I64Add);
            if byte_view {
                match source_bound {
                    mncs_model::SequenceBound::Exact(_) => {
                        body.push(Instr::I64Const(i64::MIN));
                    }
                    mncs_model::SequenceBound::UpTo(_) => {
                        body.push(Instr::LocalGet(seq));
                        body.push(Instr::I64Const(i64::MIN));
                        body.push(Instr::I64And);
                    }
                    mncs_model::SequenceBound::Param(_)
                    | mncs_model::SequenceBound::UpToParam(_) => {
                        unreachable!(
                            "generic SequenceBound must be specialized before backend lowering"
                        )
                    }
                }
                body.push(Instr::I64Or);
            }
            // Keep only the low 32 bits of the address half, preserving the
            // internal cell marker in bit 63 (a plain 0xFFFF_FFFF mask would
            // strip it right after it was set above).
            body.push(Instr::I64Const(0x8000_0000_ffff_ffffu64 as i64));
            body.push(Instr::I64And);
            body.push(Instr::LocalGet(end));
            body.push(Instr::LocalGet(start));
            body.push(Instr::I64Sub);
            body.push(Instr::I64Const(32));
            body.push(Instr::I64Shl);
            body.push(Instr::I64Or);
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::FinitePayloadProject {
            type_identity,
            discriminant,
            field,
            ..
        } => {
            let Some(variants) = composites.boxed_finites.get(type_identity) else {
                return Err(
                    "payload projection requires the payload-bearing realization".to_owned(),
                );
            };
            let fields = variants.get(discriminant).ok_or_else(|| {
                format!("finite variant {discriminant} has no declared payload layout")
            })?;
            let index = fields
                .iter()
                .position(|(name, _)| name == field)
                .ok_or_else(|| format!("variant payload has no field {field:?}"))?;
            let dest = dest_local(layout, instruction)?;
            let value = operand_local(layout, instruction, 0)?;
            body.push(Instr::LocalGet(value));
            emit_offset(body, ((index + 1) * 8) as i32);
            load_instr(fields[index].1, body);
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::FiniteIsVariant {
            type_identity,
            discriminant,
            ..
        } => {
            let dest = dest_local(layout, instruction)?;
            let value = operand_local(layout, instruction, 0)?;
            if composites.is_boxed_finite(type_identity) {
                body.push(Instr::LocalGet(value));
                body.push(Instr::I32Load);
                body.push(Instr::I32Const(*discriminant as i32));
            } else {
                body.push(Instr::LocalGet(value));
                body.push(Instr::I32Const(*discriminant as i32));
            }
            body.push(Instr::I32Eq);
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::Call { function, .. } => {
            let dest = dest_local(layout, instruction)?;
            for input in &instruction.inputs {
                body.push(Instr::LocalGet(local(layout, input)?));
            }
            let callee = layout
                .functions
                .get(function)
                .copied()
                .ok_or_else(|| "SSA call target is not in the WASM module".to_owned())?;
            body.push(Instr::Call(callee));
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::Effect => {
            return Err("effects are unsupported on the portable WASM MVP backend".to_owned());
        }
        SsaInstructionKind::HostCall { .. } => {
            return Err(
                "host calls are unsupported on the portable WASM MVP backend; run on the research bytecode backend with an explicit grant"
                    .to_owned(),
            );
        }
        SsaInstructionKind::RuntimeCheck { .. } => {
            return Err(
                "runtime checks have no executable condition in the current SSA subset".to_owned(),
            );
        }
        SsaInstructionKind::RecordConstruct {
            type_identity,
            field_names,
            ..
        } => {
            let Some(fields) = composites.records.get(type_identity) else {
                return Err(format!(
                    "record construction names an unknown record layout {type_identity}"
                ));
            };
            if fields.len() != field_names.len() || fields.len() != instruction.inputs.len() {
                return Err("record construction does not match its declared layout".to_owned());
            }
            let dest = dest_local(layout, instruction)?;
            emit_alloc(body, dest, (fields.len() as u32) * 8)?;
            for (index, (name, width)) in fields.iter().enumerate() {
                let declared = &field_names[index];
                if name != declared {
                    return Err(format!(
                        "record construction field {declared:?} does not match the canonical layout {name:?}"
                    ));
                }
                let operand = operand_local(layout, instruction, index)?;
                body.push(Instr::LocalGet(dest));
                emit_offset(body, (index * 8) as i32);
                body.push(Instr::LocalGet(operand));
                store_width(*width, body);
            }
        }
        SsaInstructionKind::RecordProject {
            type_identity,
            field,
            ..
        } => {
            let Some(fields) = composites.records.get(type_identity) else {
                return Err(format!(
                    "record projection names an unknown record layout {type_identity}"
                ));
            };
            let index = fields
                .iter()
                .position(|(name, _)| name == field)
                .ok_or_else(|| format!("record layout has no field {field:?}"))?;
            let dest = dest_local(layout, instruction)?;
            let value = operand_local(layout, instruction, 0)?;
            body.push(Instr::LocalGet(value));
            emit_offset(body, (index * 8) as i32);
            load_instr(fields[index].1, body);
            body.push(Instr::LocalSet(dest));
        }
    }
    Ok(())
}

fn lower_terminator(
    layout: &FunctionLayout,
    terminator: &SsaTerminator,
    body: &mut Vec<Instr>,
    regions: &RegionEmit<'_>,
    source_index: u32,
) -> Result<(), String> {
    match terminator {
        SsaTerminator::Return { values } => {
            let value = values
                .first()
                .ok_or_else(|| "return terminator has no value".to_owned())?;
            body.push(Instr::LocalGet(local(layout, value)?));
            body.push(Instr::Return);
        }
        SsaTerminator::Branch { target, arguments } => {
            emit_block_args(layout, target, arguments, body)?;
            emit_backedge_if_planned(layout, regions, source_index, target, body)?;
            emit_exits_if_planned(layout, regions, source_index, target, body)?;
            emit_goto(layout, target, body)?;
            body.push(Instr::Br(1));
        }
        SsaTerminator::ConditionalBranch {
            condition,
            then_target,
            then_arguments,
            else_target,
            else_arguments,
        } => {
            body.push(Instr::LocalGet(local(layout, condition)?));
            body.push(Instr::If);
            emit_block_args(layout, then_target, then_arguments, body)?;
            emit_backedge_if_planned(layout, regions, source_index, then_target, body)?;
            emit_exits_if_planned(layout, regions, source_index, then_target, body)?;
            emit_goto(layout, then_target, body)?;
            body.push(Instr::Else);
            emit_block_args(layout, else_target, else_arguments, body)?;
            emit_backedge_if_planned(layout, regions, source_index, else_target, body)?;
            emit_exits_if_planned(layout, regions, source_index, else_target, body)?;
            emit_goto(layout, else_target, body)?;
            body.push(Instr::End);
            body.push(Instr::Br(1));
        }
        SsaTerminator::Failure { mode } => {
            let _ = mode;
            body.push(Instr::Unreachable);
        }
    }
    Ok(())
}

/// Deactivate every region this edge exits (its target lies outside the
/// region body). The next entry re-pushes a fresh mark. Backedges and
/// intra-body edges take no action here.
fn emit_exits_if_planned(
    layout: &FunctionLayout,
    regions: &RegionEmit<'_>,
    source_index: u32,
    target: &SemanticId,
    body: &mut Vec<Instr>,
) -> Result<(), String> {
    let target_index = *layout
        .blocks
        .get(target)
        .ok_or_else(|| "branch target is missing".to_owned())?;
    if let Some(edges) = regions.plan.exits.get(&source_index) {
        for (exit_target, active_global) in edges {
            if *exit_target == target_index {
                body.push(Instr::I32Const(0));
                body.push(Instr::GlobalSet(*active_global));
            }
        }
    }
    Ok(())
}

/// Emit the loop-region sequence when this edge is a planned backedge
/// (its target was already emitted and carries a region plan). The
/// argument move has already run, so the header's live-in cells sit in
/// their downstream locals; forward edges take no action.
fn emit_backedge_if_planned(
    layout: &FunctionLayout,
    regions: &RegionEmit<'_>,
    source_index: u32,
    target: &SemanticId,
    body: &mut Vec<Instr>,
) -> Result<(), String> {
    let target_index = *layout
        .blocks
        .get(target)
        .ok_or_else(|| "branch target is missing".to_owned())?;
    if target_index <= source_index {
        if let Some(header) = regions.plan.headers.get(&target_index) {
            emit_backedge_region(body, regions, header)?;
        }
    }
    Ok(())
}

fn emit_block_args(
    layout: &FunctionLayout,
    target: &SemanticId,
    arguments: &[SemanticId],
    body: &mut Vec<Instr>,
) -> Result<(), String> {
    let params = layout
        .block_params
        .get(target)
        .ok_or_else(|| "branch target is missing".to_owned())?;
    if params.len() != arguments.len() {
        return Err("branch arguments do not match target block parameters".to_owned());
    }
    for argument in arguments {
        body.push(Instr::LocalGet(local(layout, argument)?));
    }
    for param in params.iter().rev() {
        body.push(Instr::LocalSet(local(layout, param)?));
    }
    Ok(())
}

fn emit_goto(
    layout: &FunctionLayout,
    target: &SemanticId,
    body: &mut Vec<Instr>,
) -> Result<(), String> {
    let index = *layout
        .blocks
        .get(target)
        .ok_or_else(|| "branch target is missing".to_owned())?;
    body.push(Instr::I32Const(index as i32));
    body.push(Instr::LocalSet(layout.pc));
    Ok(())
}

fn emit_const(body: &mut Vec<Instr>, value: i128, ty: &IrType) -> Result<(), String> {
    let (wasm_type, integer) = wasm_type(ty)?;
    match wasm_type {
        ValType::I32 => {
            let encoded = match integer {
                Some(integer) if !integer.signed => {
                    if value < 0 || value >= (1_i128 << integer.bits) {
                        return Err("unsigned constant is outside its declared width".to_owned());
                    }
                    value as u32 as i32
                }
                _ => i32::try_from(value)
                    .map_err(|_| "constant does not fit in wasm i32".to_owned())?,
            };
            body.push(Instr::I32Const(encoded));
        }
        ValType::I64 => {
            let encoded = match integer {
                Some(integer) if !integer.signed => {
                    if value < 0 || value >= (1_i128 << integer.bits) {
                        return Err("unsigned constant is outside its declared width".to_owned());
                    }
                    value as u64 as i64
                }
                _ => i64::try_from(value)
                    .map_err(|_| "constant does not fit in wasm i64".to_owned())?,
            };
            body.push(Instr::I64Const(encoded));
        }
        // Integer constants never carry a float type: floats take the
        // FloatConstant path instead.
        ValType::F64 => {
            return Err("float constants need a float literal".to_owned());
        }
    }
    Ok(())
}

fn emit_integer(
    body: &mut Vec<Instr>,
    operator: &str,
    operand_type: IntegerType,
    result_type: IntegerType,
    intent: ArithmeticIntent,
    locals: [u32; 3],
) -> Result<(), String> {
    let [left, right, dest] = locals;
    let wasm = val_type(operand_type)?;
    let unsigned = !operand_type.signed;
    let wrapping = match (wasm, operator, unsigned) {
        (ValType::I32, "add", _) => Instr::I32Add,
        (ValType::I32, "sub", _) => Instr::I32Sub,
        (ValType::I32, "mul", _) => Instr::I32Mul,
        (ValType::I32, "div", false) => Instr::I32DivS,
        (ValType::I32, "div", true) => Instr::I32DivU,
        (ValType::I32, "mod", false) => Instr::I32RemS,
        (ValType::I32, "mod", true) => Instr::I32RemU,
        (ValType::I32, "and", _) => Instr::I32And,
        (ValType::I32, "or", _) => Instr::I32Or,
        (ValType::I32, "xor", _) => Instr::I32Xor,
        // Total shift semantics: counts are modulo the declared width and
        // signed shr is arithmetic. The flag is `unsigned`, so the signed
        // (false) arm takes ShrS and the unsigned (true) arm takes ShrU.
        (ValType::I32, "shl", _) => Instr::I32Shl,
        (ValType::I32, "shr", false) => Instr::I32ShrS,
        (ValType::I32, "shr", true) => Instr::I32ShrU,
        (ValType::I64, "shl", _) => Instr::I64Shl,
        (ValType::I64, "shr", false) => Instr::I64ShrS,
        (ValType::I64, "shr", true) => Instr::I64ShrU,
        (ValType::I64, "add", _) => Instr::I64Add,
        (ValType::I64, "sub", _) => Instr::I64Sub,
        (ValType::I64, "mul", _) => Instr::I64Mul,
        (ValType::I64, "div", false) => Instr::I64DivS,
        (ValType::I64, "div", true) => Instr::I64DivU,
        (ValType::I64, "mod", false) => Instr::I64RemS,
        (ValType::I64, "mod", true) => Instr::I64RemU,
        (ValType::I64, "and", _) => Instr::I64And,
        (ValType::I64, "or", _) => Instr::I64Or,
        (ValType::I64, "xor", _) => Instr::I64Xor,
        _ => return Err(format!("unsupported integer operator {operator}")),
    };
    // Body validation admits shifts only under wrapping intent; anything
    // else keeps the historical lowering path below.
    if matches!(operator, "shl" | "shr") && matches!(intent, ArithmeticIntent::Wrapping) {
        // Total shift semantics: the u64 count reduces modulo the declared
        // value width (native shifts only mask to 32/64 lanes, which is
        // wrong for narrow widths), and signed shr is arithmetic.
        body.push(Instr::LocalGet(left));
        body.push(Instr::LocalGet(right));
        if matches!(wasm, ValType::I32) {
            body.push(Instr::I32WrapI64);
            body.push(Instr::I32Const(i32::from(operand_type.bits) - 1));
            body.push(Instr::I32And);
        }
        body.push(wrapping);
        mask_width(body, result_type, wasm);
        body.push(Instr::LocalSet(dest));
        return Ok(());
    }
    match intent {
        ArithmeticIntent::Wrapping => {
            body.push(Instr::LocalGet(left));
            body.push(Instr::LocalGet(right));
            body.push(wrapping);
            mask_width(body, result_type, wasm);
            body.push(Instr::LocalSet(dest));
        }
        ArithmeticIntent::Checked | ArithmeticIntent::Trapping => {
            body.push(Instr::LocalGet(left));
            body.push(Instr::LocalGet(right));
            body.push(wrapping);
            body.push(Instr::LocalSet(dest));
            emit_checked(body, operator, operand_type, wasm, left, right, dest)?;
            // Narrow results are normalized through an explicit local round
            // trip; a bare mask here would pop an empty operand stack.
            if result_type.bits < 32 && matches!(wasm, ValType::I32) {
                body.push(Instr::LocalGet(dest));
                mask_width(body, result_type, wasm);
                body.push(Instr::LocalSet(dest));
            }
        }
        ArithmeticIntent::Saturating => {
            if matches!(val_type(result_type)?, ValType::I64) {
                emit_saturating_wide(body, operator, operand_type, left, right, dest)?
            } else {
                emit_saturating(body, operator, operand_type, result_type, left, right, dest)?
            }
        }
        ArithmeticIntent::Widening { .. } => {
            emit_widening(body, operator, operand_type, result_type, left, right, dest)?
        }
    }
    Ok(())
}

/// Checked division/remainder: guard against division by zero (both) and
/// `MIN / -1` (division only), then use the native trapping operations.
/// Guards branch to an explicit trap so failure statuses match the reference
/// executors' runtime-failure semantics.
fn emit_checked_division(
    body: &mut Vec<Instr>,
    operator: &str,
    ty: IntegerType,
    wasm: ValType,
    left: u32,
    right: u32,
    dest: u32,
) -> Result<(), String> {
    let is_div = operator == "div";
    // MIN / -1 is a signed-only overflow case; unsigned division has no
    // such corner and must not trap on it.
    let signed = ty.signed;
    // if divisor == 0 { trap } — the zero test must match the operand's
    // carrier width; an i32 test on an i64 divisor misses high-word values.
    body.push(Instr::LocalGet(right));
    body.push(match wasm {
        ValType::I64 => Instr::I64Eqz,
        ValType::I32 => Instr::I32Eqz,
        // Float division never reaches the integer zero-guard: floats take
        // the guarded F64Div path instead.
        ValType::F64 => {
            return Err("float division needs the float path".to_owned());
        }
    });
    body.push(Instr::If);
    body.push(Instr::Unreachable);
    body.push(Instr::End);
    if is_div && signed {
        // if dividend == MIN && divisor == -1 { trap }
        let (min_const, minus_one): (Instr, Instr) = match wasm {
            ValType::I32 => (Instr::I32Const(i32::MIN), Instr::I32Const(-1)),
            ValType::I64 => (Instr::I64Const(i64::MIN), Instr::I64Const(-1)),
            ValType::F64 => {
                return Err("float division needs the float path".to_owned());
            }
        };
        let eq: Instr = match wasm {
            ValType::I32 => Instr::I32Eq,
            ValType::I64 => Instr::I64Eq,
            ValType::F64 => {
                return Err("float division needs the float path".to_owned());
            }
        };
        let eq2: Instr = match wasm {
            ValType::I32 => Instr::I32Eq,
            ValType::I64 => Instr::I64Eq,
            ValType::F64 => {
                return Err("float division needs the float path".to_owned());
            }
        };
        body.push(Instr::LocalGet(right));
        body.push(minus_one);
        body.push(eq);
        body.push(Instr::If);
        {
            body.push(Instr::LocalGet(left));
            body.push(min_const);
            body.push(eq2);
            body.push(Instr::If);
            body.push(Instr::Unreachable);
            body.push(Instr::End);
        }
        body.push(Instr::End);
    }
    body.push(Instr::LocalGet(left));
    body.push(Instr::LocalGet(right));
    // `signed` selects the signed-division instructions; unsigned operands
    // must use div.u/rem.u so their modular domain is preserved.
    let native = match (wasm, operator, signed) {
        (ValType::I32, "div", true) => Instr::I32DivS,
        (ValType::I32, "div", false) => Instr::I32DivU,
        (ValType::I32, "mod", true) => Instr::I32RemS,
        (ValType::I32, "mod", false) => Instr::I32RemU,
        (ValType::I64, "div", true) => Instr::I64DivS,
        (ValType::I64, "div", false) => Instr::I64DivU,
        (ValType::I64, "mod", true) => Instr::I64RemS,
        (ValType::I64, "mod", false) => Instr::I64RemU,
        _ => return Err(format!("unsupported division operator {operator}")),
    };
    body.push(native);
    body.push(Instr::LocalSet(dest));
    Ok(())
}

/// Emit `if !finite(local) { trap }` for one f64 local (Profile 0.12).
/// Finiteness is `local - local == 0`: exact for finite values (same
/// operand, no rounding), NaN for infinities and NaN. The trap is the
/// conservative fallback for the `float-finite` obligation.
fn emit_finite_guard(body: &mut Vec<Instr>, local: u32) {
    body.push(Instr::LocalGet(local));
    body.push(Instr::LocalGet(local));
    body.push(Instr::F64Sub);
    body.push(Instr::F64Const(0.0f64.to_bits()));
    body.push(Instr::F64Eq);
    body.push(Instr::I32Eqz);
    body.push(Instr::If);
    body.push(Instr::Unreachable);
    body.push(Instr::End);
}

/// Trap unless the i32 cell in `local` (a just-truncated float) lies in
/// the narrow integer domain `[lo, hi)`. Signedness follows the target:
/// the truncated value compares exactly, so this matches the reference
/// trunc-then-range-check verdict on every input.
fn narrow_float_trap(body: &mut Vec<Instr>, local: u32, bits: u16, signed: bool) {
    let (lo, hi) = if signed {
        (-(1_i32 << (bits - 1)), 1_i32 << (bits - 1))
    } else {
        (0, 1_i32 << bits)
    };
    let (lt, ge) = if signed {
        (Instr::I32LtS, Instr::I32GeS)
    } else {
        (Instr::I32LtU, Instr::I32GeU)
    };
    body.push(Instr::LocalGet(local));
    body.push(Instr::I32Const(lo));
    body.push(lt);
    body.push(Instr::If);
    body.push(Instr::Unreachable);
    body.push(Instr::End);
    body.push(Instr::LocalGet(local));
    body.push(Instr::I32Const(hi));
    body.push(ge);
    body.push(Instr::If);
    body.push(Instr::Unreachable);
    body.push(Instr::End);
}

fn emit_wide_i64_operation(
    body: &mut Vec<Instr>,
    operator: &str,
    ty: IntegerType,
    left: u32,
    right: u32,
) -> Result<(), String> {
    body.push(Instr::LocalGet(left));
    body.push(if ty.signed {
        Instr::I64ExtendI32S
    } else {
        Instr::I64ExtendI32U
    });
    body.push(Instr::LocalGet(right));
    body.push(if ty.signed {
        Instr::I64ExtendI32S
    } else {
        Instr::I64ExtendI32U
    });
    body.push(match operator {
        "add" => Instr::I64Add,
        "sub" => Instr::I64Sub,
        "mul" => Instr::I64Mul,
        _ => return Err(format!("unsupported exact arithmetic operator {operator}")),
    });
    Ok(())
}

/// 64-bit saturating arithmetic in i64 cells. The wrapping i64 result is
/// computed first, then overflow is detected exactly:
/// - signed add overflows when `~(l ^ r) & (l ^ w)` is negative;
/// - signed sub overflows when `(l ^ r) & (l ^ w)` is negative;
/// - unsigned add/sub compare the wrapped value against its inputs;
/// - signed mul guards the divisor (`l == 0` cannot overflow, `l == -1`
///   overflows only for `r == MIN`) before dividing, so no native trap is
///   reachable and failure statuses stay language-owned.
fn emit_saturating_wide(
    body: &mut Vec<Instr>,
    operator: &str,
    operand: IntegerType,
    left: u32,
    right: u32,
    dest: u32,
) -> Result<(), String> {
    if !matches!(operator, "add" | "sub" | "mul") {
        return Err("portable WASM saturating arithmetic realizes add, sub, and mul".to_owned());
    }
    let signed = operand.signed;
    let min = if signed { i64::MIN } else { 0_i64 };
    let max = if signed { i64::MAX } else { u64::MAX as i64 };
    body.push(Instr::LocalGet(left));
    body.push(Instr::LocalGet(right));
    match operator {
        "sub" => body.push(Instr::I64Sub),
        "mul" => body.push(Instr::I64Mul),
        _ => body.push(Instr::I64Add),
    }
    body.push(Instr::LocalSet(dest));

    let emit_overflow_condition = |body: &mut Vec<Instr>| {
        match (operator, signed) {
            ("add", true) => {
                // ~(l ^ r) & (l ^ w) < 0
                body.push(Instr::LocalGet(left));
                body.push(Instr::LocalGet(right));
                body.push(Instr::I64Xor);
                body.push(Instr::I64Const(-1));
                body.push(Instr::I64Xor);
                body.push(Instr::LocalGet(left));
                body.push(Instr::LocalGet(dest));
                body.push(Instr::I64Xor);
                body.push(Instr::I64And);
                body.push(Instr::I64Const(0));
                body.push(Instr::I64LtS);
            }
            ("sub", true) => {
                // (l ^ r) & (l ^ w) < 0
                body.push(Instr::LocalGet(left));
                body.push(Instr::LocalGet(right));
                body.push(Instr::I64Xor);
                body.push(Instr::LocalGet(left));
                body.push(Instr::LocalGet(dest));
                body.push(Instr::I64Xor);
                body.push(Instr::I64And);
                body.push(Instr::I64Const(0));
                body.push(Instr::I64LtS);
            }
            ("add", false) => {
                // w < l (unsigned wraparound)
                body.push(Instr::LocalGet(dest));
                body.push(Instr::LocalGet(left));
                body.push(Instr::I64LtU);
            }
            ("sub", false) => {
                // r > l (unsigned underflow)
                body.push(Instr::LocalGet(right));
                body.push(Instr::LocalGet(left));
                body.push(Instr::I64GtU);
            }
            _ => unreachable!("mul handled separately"),
        }
    };

    if operator == "mul" {
        if !signed {
            return Err("portable WASM realizes 64-bit saturating multiplication for signed operands; unsigned 64-bit saturating multiplication awaits a wide-multiply lowering".to_owned());
        }
        // l == 0: product cannot overflow.
        body.push(Instr::LocalGet(left));
        body.push(Instr::I64Eqz);
        body.push(Instr::If);
        body.push(Instr::Else);
        // l == -1: overflow exactly when r == MIN.
        body.push(Instr::LocalGet(left));
        body.push(Instr::I64Const(-1));
        body.push(Instr::I64Eq);
        body.push(Instr::If);
        body.push(Instr::LocalGet(right));
        body.push(Instr::I64Const(i64::MIN));
        body.push(Instr::I64Eq);
        body.push(Instr::If);
        // Same sign (both negative): upward saturation.
        body.push(Instr::I64Const(max));
        body.push(Instr::LocalSet(dest));
        body.push(Instr::End);
        body.push(Instr::Else);
        // Generic case: l ∉ {0, -1} so the division cannot trap;
        // overflow iff w / l != r. Saturation direction is downward when
        // the operand signs differ.
        body.push(Instr::LocalGet(dest));
        body.push(Instr::LocalGet(left));
        body.push(Instr::I64DivS);
        body.push(Instr::LocalGet(right));
        body.push(Instr::I64Ne);
        body.push(Instr::If);
        body.push(Instr::I64Const(min));
        body.push(Instr::I64Const(max));
        body.push(Instr::LocalGet(left));
        body.push(Instr::I64Const(63));
        body.push(Instr::I64ShrS);
        body.push(Instr::I64Const(0));
        body.push(Instr::I64Ne);
        body.push(Instr::LocalGet(right));
        body.push(Instr::I64Const(63));
        body.push(Instr::I64ShrS);
        body.push(Instr::I64Const(0));
        body.push(Instr::I64Ne);
        body.push(Instr::I32Xor);
        body.push(Instr::Select);
        body.push(Instr::LocalSet(dest));
        body.push(Instr::End);
        body.push(Instr::End);
        body.push(Instr::End);
        return Ok(());
    }

    emit_overflow_condition(body);
    body.push(Instr::If);
    if signed {
        // Direction: add saturates downward when l is negative; sub
        // saturates downward when r is positive. Select picks the first
        // value when its condition is nonzero.
        let direction_source = if operator == "add" { left } else { right };
        let downward_when_negative = operator == "add";
        body.push(Instr::I64Const(min));
        body.push(Instr::I64Const(max));
        body.push(Instr::LocalGet(direction_source));
        body.push(Instr::I64Const(63));
        body.push(Instr::I64ShrS);
        body.push(Instr::I64Const(0));
        body.push(Instr::I64Ne);
        if !downward_when_negative {
            // For sub, positive r means downward saturation: invert.
            body.push(Instr::I32Eqz);
        }
        body.push(Instr::Select);
    } else {
        body.push(Instr::I64Const(if operator == "add" { max } else { min }));
    }
    body.push(Instr::LocalSet(dest));
    body.push(Instr::End);
    Ok(())
}

fn emit_saturating(
    body: &mut Vec<Instr>,
    operator: &str,
    operand: IntegerType,
    result: IntegerType,
    left: u32,
    right: u32,
    dest: u32,
) -> Result<(), String> {
    if result != operand
        || operand.bits > 32
        || (!operand.signed && operand.bits > 31)
        || !matches!(operator, "add" | "sub" | "mul")
    {
        return Err("portable WASM saturating arithmetic requires signed 1..=32-bit or unsigned 1..=31-bit add/sub/mul in i32 cells".to_owned());
    }
    let (min, max) = if operand.signed {
        let top = 1_i64 << (operand.bits - 1);
        (-top, top - 1)
    } else {
        (0, (1_i64 << operand.bits) - 1)
    };
    emit_wide_i64_operation(body, operator, operand, left, right)?;
    body.push(Instr::I64Const(max));
    body.push(Instr::I64GtS);
    body.push(Instr::If);
    body.push(Instr::I32Const(
        i32::try_from(max).map_err(|_| "saturation maximum is not i32")?,
    ));
    body.push(Instr::LocalSet(dest));
    body.push(Instr::Else);
    emit_wide_i64_operation(body, operator, operand, left, right)?;
    body.push(Instr::I64Const(min));
    body.push(Instr::I64LtS);
    body.push(Instr::If);
    body.push(Instr::I32Const(
        i32::try_from(min).map_err(|_| "saturation minimum is not i32")?,
    ));
    body.push(Instr::LocalSet(dest));
    body.push(Instr::Else);
    emit_wide_i64_operation(body, operator, operand, left, right)?;
    body.push(Instr::I32WrapI64);
    mask_width(body, result, ValType::I32);
    body.push(Instr::LocalSet(dest));
    body.push(Instr::End);
    body.push(Instr::End);
    Ok(())
}

fn emit_widening(
    body: &mut Vec<Instr>,
    operator: &str,
    operand: IntegerType,
    result: IntegerType,
    left: u32,
    right: u32,
    dest: u32,
) -> Result<(), String> {
    if operand.bits > 32 || result.bits > 64 || result.signed != operand.signed {
        return Err(
            "portable WASM widening arithmetic requires operands <= 32 bits and results <= 64 bits"
                .to_owned(),
        );
    }
    if mncs_model::arithmetic_result_type(
        operator,
        operand,
        ArithmeticIntent::Widening { bits: result.bits },
    ) != Some(result)
    {
        return Err("widening result type is not exact for this operation".to_owned());
    }
    emit_wide_i64_operation(body, operator, operand, left, right)?;
    if matches!(val_type(result)?, ValType::I32) {
        body.push(Instr::I32WrapI64);
        mask_width(body, result, ValType::I32);
    }
    body.push(Instr::LocalSet(dest));
    Ok(())
}

fn emit_checked(
    body: &mut Vec<Instr>,
    operator: &str,
    ty: IntegerType,
    wasm: ValType,
    left: u32,
    right: u32,
    dest: u32,
) -> Result<(), String> {
    if matches!(operator, "div" | "mod") {
        emit_checked_division(body, operator, ty, wasm, left, right, dest)?;
        return Ok(());
    }
    if !matches!(operator, "add" | "sub" | "mul") {
        return Err(format!(
            "checked integer operator {operator} is unsupported on the portable WASM MVP backend"
        ));
    }

    match wasm {
        ValType::I32 if ty.signed => {
            let max = max_signed(ty.bits);
            let min = -max - 1;
            emit_range_trap_i64(body, left, right, operator, min, max, true)
        }
        ValType::I32 => emit_unsigned_trap_i32(body, left, right, dest, operator, ty.bits),
        ValType::I64 if ty.signed => {
            emit_signed_i64_overflow_trap(body, left, right, dest, operator)
        }
        ValType::I64 => emit_unsigned_i64_overflow_trap(body, left, right, dest, operator),
        // Checked float arithmetic is refused in C1: overflow is not an
        // error for binary64, and trapping needs the operand-guard path.
        ValType::F64 => Err("checked float arithmetic is unsupported".to_owned()),
    }
}

fn emit_range_trap_i64(
    body: &mut Vec<Instr>,
    left: u32,
    right: u32,
    operator: &str,
    min: i128,
    max: i128,
    signed_extend: bool,
) -> Result<(), String> {
    body.push(Instr::LocalGet(left));
    if signed_extend {
        body.push(Instr::I64ExtendI32S);
    } else {
        body.push(Instr::I64ExtendI32U);
    }
    body.push(Instr::LocalGet(right));
    if signed_extend {
        body.push(Instr::I64ExtendI32S);
    } else {
        body.push(Instr::I64ExtendI32U);
    }
    body.push(match operator {
        "add" => Instr::I64Add,
        "sub" => Instr::I64Sub,
        _ => Instr::I64Mul,
    });
    body.push(Instr::I64Const(i64::try_from(max).unwrap_or(i64::MAX)));
    body.push(Instr::I64GtS);
    body.push(Instr::LocalGet(left));
    if signed_extend {
        body.push(Instr::I64ExtendI32S);
    } else {
        body.push(Instr::I64ExtendI32U);
    }
    body.push(Instr::LocalGet(right));
    if signed_extend {
        body.push(Instr::I64ExtendI32S);
    } else {
        body.push(Instr::I64ExtendI32U);
    }
    body.push(match operator {
        "add" => Instr::I64Add,
        "sub" => Instr::I64Sub,
        _ => Instr::I64Mul,
    });
    body.push(Instr::I64Const(i64::try_from(min).unwrap_or(i64::MIN)));
    body.push(Instr::I64LtS);
    body.push(Instr::I32Or);
    body.push(Instr::If);
    body.push(Instr::Unreachable);
    body.push(Instr::End);
    Ok(())
}

fn emit_unsigned_trap_i32(
    body: &mut Vec<Instr>,
    left: u32,
    right: u32,
    dest: u32,
    operator: &str,
    bits: u16,
) -> Result<(), String> {
    let max = (1_i64 << bits) - 1;
    body.push(Instr::LocalGet(left));
    body.push(Instr::I64ExtendI32U);
    body.push(Instr::LocalGet(right));
    body.push(Instr::I64ExtendI32U);
    body.push(match operator {
        "add" => Instr::I64Add,
        "sub" => Instr::I64Sub,
        _ => Instr::I64Mul,
    });
    body.push(Instr::I64Const(max));
    body.push(Instr::I64GtU);
    body.push(Instr::If);
    body.push(Instr::Unreachable);
    body.push(Instr::End);
    let _ = dest;
    Ok(())
}

fn emit_signed_i64_overflow_trap(
    body: &mut Vec<Instr>,
    left: u32,
    right: u32,
    dest: u32,
    operator: &str,
) -> Result<(), String> {
    if operator == "mul" {
        // For l == 0 the product cannot overflow. For l == -1 the only
        // overflowing product is MIN * -1; all other nonzero divisors are
        // safe for the quotient test because MIN / -1 is the sole trapping
        // signed i64 division case.
        body.push(Instr::LocalGet(left));
        body.push(Instr::I64Eqz);
        body.push(Instr::If);
        body.push(Instr::Else);
        body.push(Instr::LocalGet(left));
        body.push(Instr::I64Const(-1));
        body.push(Instr::I64Eq);
        body.push(Instr::If);
        body.push(Instr::LocalGet(right));
        body.push(Instr::I64Const(i64::MIN));
        body.push(Instr::I64Eq);
        body.push(Instr::If);
        body.push(Instr::Unreachable);
        body.push(Instr::End);
        body.push(Instr::Else);
        body.push(Instr::LocalGet(dest));
        body.push(Instr::LocalGet(left));
        body.push(Instr::I64DivS);
        body.push(Instr::LocalGet(right));
        body.push(Instr::I64Ne);
        body.push(Instr::If);
        body.push(Instr::Unreachable);
        body.push(Instr::End);
        body.push(Instr::End);
        body.push(Instr::End);
        return Ok(());
    }
    if operator != "add" && operator != "sub" {
        return Err(format!("checked i64 operator {operator} is unsupported"));
    }
    // (a ^ b) >= 0 && (a ^ result) < 0 for add; invert b for sub.
    body.push(Instr::LocalGet(left));
    body.push(Instr::LocalGet(right));
    if operator == "sub" {
        body.push(Instr::I64Const(-1));
        body.push(Instr::I64Xor);
    }
    body.push(Instr::I64Xor);
    body.push(Instr::I64Const(0));
    body.push(Instr::I64GeS);
    body.push(Instr::LocalGet(left));
    body.push(Instr::LocalGet(dest));
    body.push(Instr::I64Xor);
    body.push(Instr::I64Const(0));
    body.push(Instr::I64LtS);
    body.push(Instr::I32And);
    body.push(Instr::If);
    body.push(Instr::Unreachable);
    body.push(Instr::End);
    Ok(())
}

fn emit_unsigned_i64_overflow_trap(
    body: &mut Vec<Instr>,
    left: u32,
    right: u32,
    dest: u32,
    operator: &str,
) -> Result<(), String> {
    match operator {
        "add" => {
            body.push(Instr::LocalGet(dest));
            body.push(Instr::LocalGet(left));
            body.push(Instr::I64LtU);
            body.push(Instr::If);
            body.push(Instr::Unreachable);
            body.push(Instr::End);
        }
        "sub" => {
            body.push(Instr::LocalGet(left));
            body.push(Instr::LocalGet(dest));
            body.push(Instr::I64LtU);
            body.push(Instr::If);
            body.push(Instr::Unreachable);
            body.push(Instr::End);
        }
        "mul" => {
            // A zero multiplier cannot overflow. Otherwise, the wrapped
            // product divided by the right operand must recover the left
            // operand; unsigned division is total for every nonzero divisor.
            body.push(Instr::LocalGet(right));
            body.push(Instr::I64Eqz);
            body.push(Instr::If);
            body.push(Instr::Else);
            body.push(Instr::LocalGet(dest));
            body.push(Instr::LocalGet(right));
            body.push(Instr::I64DivU);
            body.push(Instr::LocalGet(left));
            body.push(Instr::I64Ne);
            body.push(Instr::If);
            body.push(Instr::Unreachable);
            body.push(Instr::End);
            body.push(Instr::End);
        }
        _ => {
            return Err(format!(
                "checked unsigned i64 operator {operator} is unsupported"
            ))
        }
    }
    Ok(())
}

fn mask_width(body: &mut Vec<Instr>, ty: IntegerType, wasm: ValType) {
    if ty.bits >= 32 && matches!(wasm, ValType::I32) || ty.bits >= 64 {
        return;
    }
    if matches!(wasm, ValType::I32) {
        let mask = (1_i32.wrapping_shl(u32::from(ty.bits))) - 1;
        body.push(Instr::I32Const(mask));
        body.push(Instr::I32And);
        if ty.signed {
            // Sign-extend an N-bit value without relying on optional WASM
            // sign-extension instructions: (masked ^ sign) - sign.
            let sign = 1_i32.wrapping_shl(u32::from(ty.bits - 1));
            body.push(Instr::I32Const(sign));
            body.push(Instr::I32Xor);
            body.push(Instr::I32Const(sign));
            body.push(Instr::I32Sub);
        }
    }
}

fn compare_instr(predicate: &str, ty: IntegerType) -> Result<Instr, String> {
    let wasm = val_type(ty)?;
    Ok(match (wasm, predicate, ty.signed) {
        (ValType::I32, "eq", _) => Instr::I32Eq,
        (ValType::I32, "ne", _) => Instr::I32Ne,
        (ValType::I32, "lt", true) => Instr::I32LtS,
        (ValType::I32, "lt", false) => Instr::I32LtU,
        (ValType::I32, "le", true) => Instr::I32LeS,
        (ValType::I32, "le", false) => Instr::I32LeU,
        (ValType::I32, "gt", true) => Instr::I32GtS,
        (ValType::I32, "gt", false) => Instr::I32GtU,
        (ValType::I32, "ge", true) => Instr::I32GeS,
        (ValType::I32, "ge", false) => Instr::I32GeU,
        (ValType::I64, "eq", _) => Instr::I64Eq,
        (ValType::I64, "ne", _) => Instr::I64Ne,
        (ValType::I64, "lt", true) => Instr::I64LtS,
        (ValType::I64, "lt", false) => Instr::I64LtU,
        (ValType::I64, "le", true) => Instr::I64LeS,
        (ValType::I64, "le", false) => Instr::I64LeU,
        (ValType::I64, "gt", true) => Instr::I64GtS,
        (ValType::I64, "gt", false) => Instr::I64GtU,
        (ValType::I64, "ge", true) => Instr::I64GeS,
        (ValType::I64, "ge", false) => Instr::I64GeU,
        _ => return Err(format!("unsupported comparison predicate {predicate}")),
    })
}

fn dest_local(
    layout: &FunctionLayout,
    instruction: &mncs_model::SsaInstruction,
) -> Result<u32, String> {
    let output = instruction
        .outputs
        .first()
        .ok_or_else(|| "instruction has no destination".to_owned())?;
    local(layout, &output.identity)
}

fn operand_local(
    layout: &FunctionLayout,
    instruction: &mncs_model::SsaInstruction,
    index: usize,
) -> Result<u32, String> {
    let operand = instruction
        .inputs
        .get(index)
        .ok_or_else(|| "instruction is missing an operand".to_owned())?;
    local(layout, operand)
}

fn local(layout: &FunctionLayout, identity: &SemanticId) -> Result<u32, String> {
    layout
        .values
        .get(identity)
        .copied()
        .ok_or_else(|| format!("SSA value {} is not mapped to a WASM local", identity.0))
}

/// Emit `$mncs_alloc(bytes) -> ptr` into every module that materializes
/// composites, plus the bump-pointer global it uses (global 0).
pub(crate) const ALLOC_FUNCTION_NAME: &str = "mncs_alloc";
/// Export a stable byte-buffer reservation ABI alongside the allocator.
/// The returned i64 packs the byte capacity in the high half and the linear
/// memory offset in the low half.
const HOST_BUFFER_FUNCTION_NAME: &str = "mncs_host_buffer";
const HOST_BUFFER_RESET_FUNCTION_NAME: &str = "mncs_host_buffer_reset";

pub fn emit_alloc_helpers(module: &mut WasmModule) {
    if !module.globals.is_empty() && module.memory.is_some() {
        // Global 0 is the allocator cursor. Keep the end of the host-owned
        // region in a second cursor so hosts can recycle target-array
        // allocations between calls without overwriting their input bytes.
        module.globals.push(crate::wasm::WasmGlobal {
            valtype: ValType::I32,
            mutable: true,
            init: 8,
        });
        let host_end_global = (module.globals.len() - 1) as u32;
        // $mncs_alloc(bytes): bump the global pointer, keep 8-byte alignment,
        // return the previous pointer. Appending keeps every existing callee
        // index stable.
        module.functions.push(WasmFunction {
            name: ALLOC_FUNCTION_NAME.to_owned(),
            params: vec![ValType::I32],
            results: vec![ValType::I32],
            locals: vec![ValType::I32],
            body: vec![
                Instr::GlobalGet(0),
                Instr::LocalSet(1),
                Instr::GlobalGet(0),
                Instr::LocalGet(0),
                // round the request up to an 8-byte multiple
                Instr::I32Const(7),
                Instr::I32Add,
                Instr::I32Const(-8),
                Instr::I32And,
                Instr::I32Add,
                Instr::GlobalSet(0),
                Instr::LocalGet(1),
            ],
        });
        // Defined-function indices follow the imports, so the helper
        // index shifts by the import count.
        let alloc_index = (module.imports.len() + module.functions.len() - 1) as u32;
        for function in &mut module.functions {
            rewrite_alloc_calls(function, alloc_index);
        }
        module.functions.push(WasmFunction {
            name: HOST_BUFFER_FUNCTION_NAME.to_owned(),
            params: vec![ValType::I32],
            results: vec![ValType::I64],
            locals: vec![ValType::I32],
            body: vec![
                // Reserve the host-owned region before any later composite
                // allocations and return {capacity:32, offset:32}.
                Instr::LocalGet(0),
                Instr::Call(alloc_index),
                Instr::LocalSet(1),
                Instr::GlobalGet(0),
                Instr::GlobalSet(host_end_global),
                Instr::LocalGet(1),
                Instr::I64ExtendI32U,
                Instr::LocalGet(0),
                Instr::I64ExtendI32U,
                Instr::I64Const(32),
                Instr::I64Shl,
                Instr::I64Or,
            ],
        });
        module.functions.push(WasmFunction {
            name: HOST_BUFFER_RESET_FUNCTION_NAME.to_owned(),
            params: Vec::new(),
            results: Vec::new(),
            locals: Vec::new(),
            body: vec![Instr::GlobalGet(host_end_global), Instr::GlobalSet(0)],
        });
    }
    // Any leftover marker means a lowered function asked for allocation
    // without composites being declared; fail loudly rather than encode it.
    for function in &module.functions {
        if function
            .body
            .iter()
            .any(|instr| matches!(instr, crate::wasm::Instr::AllocCall))
        {
            panic!("AllocCall marker survived lowering without arena helpers");
        }
    }
}

/// Rewrite `Call(placeholder)` markers emitted during lowering into the real
/// helper index once it is known.
fn rewrite_alloc_calls(function: &mut WasmFunction, alloc_index: u32) {
    for instr in &mut function.body {
        if matches!(instr, crate::wasm::Instr::AllocCall) {
            *instr = crate::wasm::Instr::Call(alloc_index);
        }
    }
}

/// Emit a call to `$mncs_alloc` with a constant byte count; result lands in
/// `dest`. The call is emitted as an `AllocCall` marker and rewritten when the
/// helper index is assigned at module level.
fn emit_alloc(body: &mut Vec<Instr>, dest_local: u32, bytes: u32) -> Result<(), String> {
    body.push(Instr::I32Const(bytes as i32));
    body.push(Instr::AllocCall);
    body.push(Instr::LocalSet(dest_local));
    Ok(())
}

/// Address arithmetic: push base + offset (offset may be negative-free here;
/// composite slot offsets are always multiples of 8).
fn emit_offset(body: &mut Vec<Instr>, offset: i32) {
    if offset != 0 {
        body.push(Instr::I32Const(offset));
        body.push(Instr::I32Add);
    }
}

fn load_instr(width: SlotWidth, body: &mut Vec<Instr>) {
    match width {
        SlotWidth::W32 => body.push(Instr::I32Load),
        SlotWidth::W64 => body.push(Instr::I64Load),
    }
}

fn vector_lane_realization(element: &BodyType) -> Result<(ValType, u32, IntegerType), String> {
    let BodyType::Integer(integer) = element else {
        return Err("initial vector realization supports integer lanes only".to_owned());
    };
    match integer.bits {
        32 => Ok((ValType::I32, 4, *integer)),
        64 => Ok((ValType::I64, 8, *integer)),
        bits => Err(format!(
            "portable WASM vector realization does not yet support {bits}-bit lanes"
        )),
    }
}

fn load_valtype(ty: ValType, body: &mut Vec<Instr>) {
    match ty {
        ValType::I32 => body.push(Instr::I32Load),
        ValType::I64 => body.push(Instr::I64Load),
        ValType::F64 => body.push(Instr::F64Load),
    }
}

fn store_valtype(ty: ValType, body: &mut Vec<Instr>) {
    match ty {
        ValType::I32 => body.push(Instr::I32Store),
        ValType::I64 => body.push(Instr::I64Store),
        ValType::F64 => body.push(Instr::F64Store),
    }
}

fn emit_vector_lane_check(
    body: &mut Vec<Instr>,
    index: u32,
    lanes: u32,
    evidence: &mncs_model::BoundsEvidence,
) {
    if matches!(evidence, mncs_model::BoundsEvidence::RuntimeChecked { .. }) {
        body.push(Instr::LocalGet(index));
        body.push(Instr::I64Const(i64::from(lanes)));
        body.push(Instr::I64GeU);
        body.push(Instr::If);
        body.push(Instr::Unreachable);
        body.push(Instr::End);
    }
}

fn emit_mask_limit(body: &mut Vec<Instr>, lanes: u32) {
    let mask = if lanes >= 64 {
        -1_i64
    } else {
        ((1_u64 << lanes) - 1) as i64
    };
    body.push(Instr::I64Const(mask));
}

fn emit_vector_binary_lane(
    body: &mut Vec<Instr>,
    lane_type: ValType,
    integer: IntegerType,
    operator: &str,
    left: u32,
    right: u32,
    offset: i32,
) -> Result<(), String> {
    let load = |body: &mut Vec<Instr>, local| {
        body.push(Instr::LocalGet(local));
        emit_offset(body, offset);
        load_valtype(lane_type, body);
    };
    if matches!(operator, "min" | "max") {
        // b ^ ((a ^ b) & -(a cmp b)); branchless min/max over signed or
        // unsigned lanes, where cmp is lt for min and gt for max.
        load(body, right);
        load(body, left);
        load(body, right);
        body.push(match lane_type {
            ValType::I32 => Instr::I32Xor,
            ValType::I64 => Instr::I64Xor,
            ValType::F64 => {
                return Err("float vectors are not supported".to_owned());
            }
        });
        body.push(match lane_type {
            ValType::I32 => Instr::I32Const(0),
            ValType::I64 => Instr::I64Const(0),
            ValType::F64 => {
                return Err("float vectors are not supported".to_owned());
            }
        });
        load(body, left);
        load(body, right);
        body.push(compare_instr(
            if operator == "min" { "lt" } else { "gt" },
            integer,
        )?);
        if lane_type == ValType::I64 {
            body.push(Instr::I64ExtendI32U);
        }
        body.push(match lane_type {
            ValType::I32 => Instr::I32Sub,
            ValType::I64 => Instr::I64Sub,
            ValType::F64 => {
                return Err("float vectors are not supported".to_owned());
            }
        });
        body.push(match lane_type {
            ValType::I32 => Instr::I32And,
            ValType::I64 => Instr::I64And,
            ValType::F64 => {
                return Err("float vectors are not supported".to_owned());
            }
        });
        body.push(match lane_type {
            ValType::I32 => Instr::I32Xor,
            ValType::I64 => Instr::I64Xor,
            ValType::F64 => {
                return Err("float vectors are not supported".to_owned());
            }
        });
        return Ok(());
    }
    load(body, left);
    load(body, right);
    body.push(match (lane_type, operator, integer.signed) {
        (ValType::I32, "add", _) => Instr::I32Add,
        (ValType::I32, "sub", _) => Instr::I32Sub,
        (ValType::I32, "mul", _) => Instr::I32Mul,
        (ValType::I32, "and", _) => Instr::I32And,
        (ValType::I32, "or", _) => Instr::I32Or,
        (ValType::I32, "xor", _) => Instr::I32Xor,
        (ValType::I32, "shl", _) => Instr::I32Shl,
        (ValType::I32, "shr", true) => Instr::I32ShrS,
        (ValType::I32, "shr", false) => Instr::I32ShrU,
        (ValType::I64, "add", _) => Instr::I64Add,
        (ValType::I64, "sub", _) => Instr::I64Sub,
        (ValType::I64, "mul", _) => Instr::I64Mul,
        (ValType::I64, "and", _) => Instr::I64And,
        (ValType::I64, "or", _) => Instr::I64Or,
        (ValType::I64, "xor", _) => Instr::I64Xor,
        (ValType::I64, "shl", _) => Instr::I64Shl,
        (ValType::I64, "shr", true) => Instr::I64ShrS,
        (ValType::I64, "shr", false) => Instr::I64ShrU,
        (_, other, _) => return Err(format!("unsupported vector operator {other}")),
    });
    Ok(())
}

fn emit_vector_reduce_lane(
    body: &mut Vec<Instr>,
    dest: u32,
    vector: u32,
    offset: i32,
    lane_type: ValType,
    integer: IntegerType,
    operator: &str,
) -> Result<(), String> {
    let lane = |body: &mut Vec<Instr>| {
        body.push(Instr::LocalGet(vector));
        emit_offset(body, offset);
        load_valtype(lane_type, body);
    };
    if matches!(operator, "min" | "max") {
        // Same branchless blend as lane-wise min/max: b ^ ((a ^ b) & -cmp).
        lane(body);
        body.push(Instr::LocalGet(dest));
        lane(body);
        body.push(match lane_type {
            ValType::I32 => Instr::I32Xor,
            ValType::I64 => Instr::I64Xor,
            ValType::F64 => {
                return Err("float vectors are not supported".to_owned());
            }
        });
        body.push(match lane_type {
            ValType::I32 => Instr::I32Const(0),
            ValType::I64 => Instr::I64Const(0),
            ValType::F64 => {
                return Err("float vectors are not supported".to_owned());
            }
        });
        body.push(Instr::LocalGet(dest));
        lane(body);
        body.push(compare_instr(
            if operator == "min" { "lt" } else { "gt" },
            integer,
        )?);
        if lane_type == ValType::I64 {
            body.push(Instr::I64ExtendI32U);
        }
        body.push(match lane_type {
            ValType::I32 => Instr::I32Sub,
            ValType::I64 => Instr::I64Sub,
            ValType::F64 => {
                return Err("float vectors are not supported".to_owned());
            }
        });
        body.push(match lane_type {
            ValType::I32 => Instr::I32And,
            ValType::I64 => Instr::I64And,
            ValType::F64 => {
                return Err("float vectors are not supported".to_owned());
            }
        });
        body.push(match lane_type {
            ValType::I32 => Instr::I32Xor,
            ValType::I64 => Instr::I64Xor,
            ValType::F64 => {
                return Err("float vectors are not supported".to_owned());
            }
        });
    } else if operator == "sum" {
        body.push(Instr::LocalGet(dest));
        lane(body);
        body.push(match lane_type {
            ValType::I32 => Instr::I32Add,
            ValType::I64 => Instr::I64Add,
            ValType::F64 => {
                return Err("float vectors are not supported".to_owned());
            }
        });
    } else {
        return Err(format!("unsupported vector reduction {operator}"));
    }
    body.push(Instr::LocalSet(dest));
    Ok(())
}

fn store_width(width: SlotWidth, body: &mut Vec<Instr>) {
    match width {
        SlotWidth::W32 => body.push(Instr::I32Store),
        SlotWidth::W64 => body.push(Instr::I64Store),
    }
}

/// Store instruction matching an operand's SSA value type at the top of stack.
fn store_instr(
    layout: &FunctionLayout,
    instruction: &mncs_model::SsaInstruction,
    index: usize,
    body: &mut Vec<Instr>,
) -> Result<(), String> {
    let identity = instruction
        .inputs
        .get(index)
        .ok_or_else(|| format!("payload operand {index} is missing"))?;
    let valtype = layout.types.get(identity).copied().unwrap_or(ValType::I32);
    match valtype {
        ValType::I64 => store_width(SlotWidth::W64, body),
        ValType::I32 => store_width(SlotWidth::W32, body),
        // Floats are bit-carried: an f64 payload occupies a full 8-byte
        // slot exactly like an i64 (I64Store is bitwise F64Store).
        ValType::F64 => store_width(SlotWidth::W64, body),
    }
    Ok(())
}

/// Store for one sequence element: width follows the operand's realization.
fn store_element_width(
    layout: &FunctionLayout,
    instruction: &mncs_model::SsaInstruction,
    index: usize,
    body: &mut Vec<Instr>,
) -> Result<(), String> {
    let identity = instruction
        .inputs
        .get(index)
        .ok_or_else(|| format!("sequence element {index} is missing"))?;
    let valtype = layout.types.get(identity).copied().unwrap_or(ValType::I32);
    match valtype {
        ValType::I64 => store_width(SlotWidth::W64, body),
        ValType::I32 => store_width(SlotWidth::W32, body),
        // Floats are bit-carried: an f64 element occupies a full 8-byte
        // slot exactly like an i64.
        ValType::F64 => store_width(SlotWidth::W64, body),
    }
    Ok(())
}

/// Load for one sequence element: the destination's realization type picks
/// the load width, mirroring the canonical slot contract.
fn load_element_width(
    layout: &FunctionLayout,
    instruction: &mncs_model::SsaInstruction,
    body: &mut Vec<Instr>,
) -> Result<(), String> {
    let output = instruction
        .outputs
        .first()
        .ok_or_else(|| "projection has no result".to_owned())?;
    if matches!(&output.ty, IrType::Named(name) if name == "byte") {
        // A bounded byte view is a packed host buffer, not an arena cell.
        // Read one unsigned byte so adjacent bytes in the same view do not
        // get folded into a four-byte scalar load.
        body.push(Instr::I32Load8U);
    } else {
        let valtype = layout
            .types
            .get(&output.identity)
            .copied()
            .unwrap_or(ValType::I32);
        match valtype {
            ValType::I64 => body.push(Instr::I64Load),
            ValType::I32 => body.push(Instr::I32Load),
            // Bit-carried f64: an 8-byte load moves the payload bits.
            ValType::F64 => body.push(Instr::F64Load),
        }
    }
    Ok(())
}

/// Explicit total conversion (Profile 0.7): narrowing truncates high bits;
/// widening extends by the *source* signedness (`byte` zero-extends).
fn emit_convert(
    layout: &FunctionLayout,
    instruction: &mncs_model::SsaInstruction,
    body: &mut Vec<Instr>,
    from: BodyType,
    to: BodyType,
) -> Result<(), String> {
    let dest = dest_local(layout, instruction)?;
    let src = operand_local(layout, instruction, 0)?;
    // Float edges (Profile 0.12). Only binary64 is admitted on either
    // side; anything else stays refused.
    if matches!(&from, BodyType::Float(float) if !float.is_supported())
        || matches!(&to, BodyType::Float(float) if !float.is_supported())
    {
        return Err("only binary64 float conversions are supported".to_owned());
    }
    if matches!(&to, BodyType::Float(_)) {
        // Integer, byte, and boolean domains convert to binary64 exactly
        // rounded by hardware. Narrow sources ride normalized in i32
        // cells, so they convert as i32 by source signedness.
        body.push(Instr::LocalGet(src));
        match &from {
            BodyType::Integer(ty) if ty.bits == 64 && ty.signed => {
                body.push(Instr::F64ConvertI64S);
            }
            BodyType::Integer(ty) if ty.bits == 64 => {
                body.push(Instr::F64ConvertI64U);
            }
            BodyType::Integer(ty) if ty.signed => {
                body.push(Instr::F64ConvertI32S);
            }
            BodyType::Integer(_) | BodyType::Byte => {
                body.push(Instr::F64ConvertI32U);
            }
            BodyType::Named(name) if name == "bool" => {
                body.push(Instr::F64ConvertI32U);
            }
            BodyType::Float(_) => {}
            _ => {
                return Err("conversion source cannot target a float".to_owned());
            }
        }
        body.push(Instr::LocalSet(dest));
        return Ok(());
    }
    if matches!(&from, BodyType::Float(_)) {
        // Float to integer truncates toward zero and traps on non-finite
        // or out-of-range inputs (the float trap rule). Full-width
        // targets trap in the truncations themselves; narrow targets
        // truncate to i32 first (which traps outside the i32 domain the
        // reference would also reject) and then trap outside the narrow
        // domain with integer comparisons over the truncated value.
        body.push(Instr::LocalGet(src));
        match &to {
            BodyType::Integer(ty) if ty.bits == 64 && ty.signed => {
                body.push(Instr::I64TruncF64S);
            }
            BodyType::Integer(ty) if ty.bits == 64 => {
                body.push(Instr::I64TruncF64U);
            }
            BodyType::Integer(ty) if ty.signed => {
                body.push(Instr::I32TruncF64S);
            }
            BodyType::Integer(_) => {
                body.push(Instr::I32TruncF64U);
            }
            BodyType::Byte => {
                body.push(Instr::I32TruncF64U);
            }
            _ => {
                return Err("float conversion target must be numeric".to_owned());
            }
        }
        body.push(Instr::LocalSet(dest));
        if let BodyType::Integer(ty) = &to {
            if ty.bits < 32 {
                narrow_float_trap(body, dest, ty.bits, ty.signed);
            }
        }
        if matches!(&to, BodyType::Byte) {
            narrow_float_trap(body, dest, 8, false);
        }
        return Ok(());
    }
    let src_wide = matches!(&from, BodyType::Integer(ty) if ty.bits == 64);
    let dst_wide = matches!(&to, BodyType::Integer(ty) if ty.bits == 64);
    let dst_signed = matches!(&to, BodyType::Integer(ty) if ty.signed);
    let (dst_bits, dst_is_byte) = match &to {
        BodyType::Byte => (8u16, true),
        BodyType::Integer(ty) => (ty.bits, false),
        _ => return Err("conversion target must be scalar".to_owned()),
    };
    body.push(Instr::LocalGet(src));
    if !src_wide && dst_wide {
        // Widen i32 cells into i64 by source signedness; bytes zero-extend.
        if matches!(&from, BodyType::Integer(ty) if ty.signed) {
            body.push(Instr::I64ExtendI32S);
        } else {
            body.push(Instr::I64ExtendI32U);
        }
    } else if src_wide && !dst_wide {
        body.push(Instr::I32WrapI64);
    }
    if dst_bits < 32 || dst_is_byte {
        // Narrow targets normalize inside their declared domain.
        let mask = (1_i32.checked_shl(u32::from(dst_bits)).unwrap_or(0)).wrapping_sub(1);
        body.push(Instr::I32Const(mask));
        body.push(Instr::I32And);
        if dst_signed {
            let sign = 1_i32.wrapping_shl(u32::from(dst_bits - 1));
            body.push(Instr::I32Const(sign));
            body.push(Instr::I32Xor);
            body.push(Instr::I32Const(sign));
            body.push(Instr::I32Sub);
        }
    } else if dst_wide && matches!(&to, BodyType::Integer(ty) if ty.bits < 64) {
        let mask = ((1_i128 << dst_bits) - 1) as i64;
        body.push(Instr::I64Const(mask));
        body.push(Instr::I64And);
        if dst_signed {
            let shift = 64 - i64::from(dst_bits);
            body.push(Instr::I64Const(shift));
            body.push(Instr::I64Shl);
            body.push(Instr::I64Const(shift));
            body.push(Instr::I64ShrS);
        }
    }
    body.push(Instr::LocalSet(dest));
    Ok(())
}

fn wasm_type(ty: &IrType) -> Result<(ValType, Option<IntegerType>), String> {
    match ty {
        IrType::Finite { .. } => Ok((ValType::I32, None)),
        // Records and payload-bearing finite variants are realized as pointers
        // into linear memory; the value's local holds the cell address.
        IrType::Record { .. } => Ok((ValType::I32, None)),
        IrType::Named(name) if name == "bool" => Ok((ValType::I32, None)),
        IrType::Named(name) => match BodyType::from_semantic_name(name) {
            BodyType::Float(float) if float.is_supported() => Ok((ValType::F64, None)),
            BodyType::Integer(integer) => Ok((val_type(integer)?, Some(integer))),
            // Bytes ride zero-extended in i32 cells.
            BodyType::Byte => Ok((ValType::I32, None)),
            // Exact sequences are canonical cell pointers; bounded views are
            // packed (offset | length << 32) descriptors riding i64.
            BodyType::Sequence {
                bound: mncs_model::SequenceBound::Exact(_),
                ..
            } => Ok((ValType::I32, None)),
            BodyType::Sequence {
                bound: mncs_model::SequenceBound::UpTo(_),
                ..
            } => Ok((ValType::I64, None)),
            BodyType::Sequence {
                bound: mncs_model::SequenceBound::Param(_) | mncs_model::SequenceBound::UpToParam(_),
                ..
            } => {
                Err("generic SequenceBound must be specialized before backend lowering".to_owned())
            }
            BodyType::Vector { .. } => Ok((ValType::I32, None)),
            BodyType::Mask { .. } => Ok((ValType::I64, None)),
            BodyType::Named(_) | BodyType::Finite { .. } | BodyType::Record { .. } => {
                Err(format!("unsupported SSA type {name}"))
            }
            // Non-binary64 floats stay refused in C1.
            BodyType::Float(_) => Err(format!("unsupported SSA type {name}")),
            BodyType::GenericParam { .. } => {
                Err("generic type parameter must be specialized before backend lowering".to_owned())
            }
        },
    }
}

fn val_type(ty: IntegerType) -> Result<ValType, String> {
    crate::wasm::val_type_for(ty)
        .ok_or_else(|| format!("integer width {} is unsupported on WASM MVP", ty.bits))
}

fn max_signed(bits: u16) -> i128 {
    (1_i128 << (bits - 1)) - 1
}

fn export_name(identity: &SemanticId) -> String {
    identity
        .0
        .rsplit(':')
        .next()
        .unwrap_or(identity.0.as_str())
        .to_owned()
}
