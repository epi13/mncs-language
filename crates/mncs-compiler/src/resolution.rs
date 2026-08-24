//! Authoritative source-level name resolution recorded during elaboration.
//!
//! Elaboration resolves every name occurrence against the lexical and nominal
//! namespaces of the source profiles. This module retains those decisions as a
//! reusable artifact so tools (language services, CLI navigation, Forge) can
//! navigate MNCS sources without re-implementing binding rules.
//!
//! The index records *use sites only*: every resolved name occurrence points at
//! the source span of the declaration it resolves to. Declaration inventories
//! are derivable from the AST, which remains lossless. No resolution is
//! inferred from text alone; every entry originates from the same binding
//! decision that elaboration used to accept the program.

use serde::{Deserialize, Serialize};

use mncs_syntax::SourceSpan;

/// Schema version of [`NameResolutionIndex`].
pub const NAME_RESOLUTION_SCHEMA_VERSION: &str = "0.1";

/// The kind of declaration a resolved name occurrence points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolvedNameKind {
    Function,
    Parameter,
    Binding,
    IterationState,
    FiniteType,
    FiniteVariant,
    RecordType,
    RecordField,
}

/// One resolved name occurrence: a use-site span, the declaration span it
/// binds to, and the kind of the declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NameResolution {
    /// Span of the name occurrence (the use site).
    pub occurrence: SourceSpan,
    /// Span of the resolved declaration's name.
    pub declaration: SourceSpan,
    pub kind: ResolvedNameKind,
}

/// Deterministic collection of [`NameResolution`] entries for one source
/// document, sorted by occurrence position and deduplicated.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct NameResolutionIndex {
    pub schema_version: String,
    pub resolutions: Vec<NameResolution>,
}

impl NameResolutionIndex {
    pub fn new(resolutions: Vec<NameResolution>) -> Self {
        let mut resolutions = resolutions;
        resolutions.sort();
        resolutions.dedup();
        Self {
            schema_version: NAME_RESOLUTION_SCHEMA_VERSION.to_owned(),
            resolutions,
        }
    }

    /// All resolved occurrences whose span contains `offset`. The end bound is
    /// inclusive so that an editor cursor resting just past the final
    /// character still finds the name.
    pub fn at_offset(&self, offset: usize) -> impl Iterator<Item = &NameResolution> {
        self.resolutions.iter().filter(move |resolution| {
            resolution.occurrence.start <= offset && offset <= resolution.occurrence.end
        })
    }

    /// All resolved occurrences that point at `declaration`.
    pub fn references_to(&self, declaration: SourceSpan) -> impl Iterator<Item = &NameResolution> {
        self.resolutions
            .iter()
            .filter(move |resolution| resolution.declaration == declaration)
    }
}

#[cfg(test)]
mod tests {
    use super::{NameResolutionIndex, ResolvedNameKind};
    use crate::elaborate_program_with_resolutions;
    use mncs_syntax::{parse, SourceArtifactKind, SourceEnvelope, SourceSpan};

    /// Elaborate `source` and return its authoritative resolution index.
    fn resolutions(source: &str) -> NameResolutionIndex {
        let envelope = SourceEnvelope::inline(SourceArtifactKind::Program, "test", source);
        let parsed = parse(&envelope);
        let ast = parsed.ast.expect("fixture parses");
        let (_, recorded) = elaborate_program_with_resolutions(&ast);
        NameResolutionIndex::new(recorded)
    }

    /// Span of the `occurrence`-th occurrence of `needle` in `source`.
    fn span_at(source: &str, needle: &str, occurrence: usize) -> SourceSpan {
        let start = source
            .match_indices(needle)
            .nth(occurrence)
            .map(|(offset, _)| offset)
            .unwrap_or_else(|| panic!("missing occurrence {occurrence} of {needle:?}"));
        SourceSpan::at(source, start, start + needle.len())
    }

    fn kinds_at(
        index: &NameResolutionIndex,
        source: &str,
        needle: &str,
        occurrence: usize,
    ) -> Vec<ResolvedNameKind> {
        index
            .at_offset(span_at(source, needle, occurrence).start + 1)
            .map(|resolution| resolution.kind)
            .collect()
    }

    #[test]
    fn bindings_calls_and_types_resolve_to_their_declarations() {
        let source = "\
mncs 0.3;
module example.resolution;

enum Status { Ok, Failed }

fn helper(value: i64) -> (result: i64) {
    return value;
}

fn main(input: i64) -> (result: i64)
    requires positive
{
    let doubled: i64 = input + input;
    return helper(doubled);
}";
        let index = resolutions(source);

        // Parameter occurrences resolve to the parameter declarations.
        assert_eq!(
            kinds_at(&index, source, "input", 1),
            vec![ResolvedNameKind::Parameter]
        );
        // Local binding occurrence resolves to its own let declaration.
        assert_eq!(
            kinds_at(&index, source, "doubled", 1),
            vec![ResolvedNameKind::Binding]
        );
        // Call target resolves to the function declaration.
        assert_eq!(
            kinds_at(&index, source, "helper", 1),
            vec![ResolvedNameKind::Function]
        );
        let call = span_at(source, "helper", 1);
        let resolved = index.at_offset(call.start).copied().collect::<Vec<_>>();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].declaration, span_at(source, "helper", 0));

        // references_to finds every use of the binding.
        let declaration = span_at(source, "doubled", 0);
        let references = index.references_to(declaration).count();
        assert_eq!(references, 2); // declaration site plus return occurrence
    }

    #[test]
    fn finite_variants_match_arms_and_iteration_state_resolve() {
        let source = "\
mncs 0.4;
module example.iteration;

enum Outcome { Hit, Miss }

fn classify(attempts: i64) -> (result: i64) {
    iterate loop up_to 4 carrying state: i64 = attempts {
        next state = state;
    }
    return attempts;
}";
        let index = resolutions(source);
        assert_eq!(
            kinds_at(&index, source, "state", 2),
            vec![ResolvedNameKind::IterationState]
        );

        let variant_source = "\
mncs 0.3;
module example.variants;

enum Color { Red, Green }

fn pick(color: Color) -> (result: i64) {
    return match color { Red => 1, Green => 0 };
}";
        let variants = resolutions(variant_source);
        assert_eq!(
            kinds_at(&variants, variant_source, "Red", 1),
            vec![ResolvedNameKind::FiniteVariant]
        );
        assert_eq!(
            kinds_at(&variants, variant_source, "Color", 1),
            vec![ResolvedNameKind::FiniteType]
        );
    }

    #[test]
    fn record_fields_and_literal_types_resolve_in_profile_05() {
        let source = "\
mncs 0.5;
module example.records;

record Reading { celsius: i32 }

fn main(celsius: i32) -> (result: i32) {
    let base: Reading = Reading { celsius: celsius };
    return base.celsius;
}";
        let index = resolutions(source);
        // Annotation position resolves to the record declaration.
        assert_eq!(
            kinds_at(&index, source, "Reading", 1),
            vec![ResolvedNameKind::RecordType]
        );
        // Literal constructor name and field both resolve.
        assert!(kinds_at(&index, source, "Reading", 2).contains(&ResolvedNameKind::RecordType));
        assert_eq!(
            kinds_at(&index, source, "celsius", 2),
            vec![ResolvedNameKind::RecordField]
        );
        assert_eq!(
            kinds_at(&index, source, "celsius", 3),
            vec![ResolvedNameKind::Parameter]
        );
        // Projection field resolves too.
        assert_eq!(
            kinds_at(&index, source, "celsius", 4),
            vec![ResolvedNameKind::RecordField]
        );
    }

    #[test]
    fn unresolved_names_are_absent_and_partial_programs_keep_prior_resolutions() {
        let missing_source = "\
mncs 0.3;
module example.missing;

fn main(input: i64) -> (result: i64) {
    return ghost;
}";
        let missing = resolutions(missing_source);
        let ghost = span_at(missing_source, "ghost", 0);
        // `ghost` does not resolve to any declaration.
        assert!(missing.at_offset(ghost.start).next().is_none());

        let partial_source = "\
mncs 0.3;
module example.partial;

fn good(input: i64) -> (result: i64) {
    return input;
}

fn broken(input: i64) -> (result: i64) {
    return ghost;
}";
        let partial = resolutions(partial_source);
        // Resolutions from the valid function are retained even though the
        // program fails to elaborate as a whole.
        assert_eq!(
            kinds_at(&partial, partial_source, "input", 1),
            vec![ResolvedNameKind::Parameter]
        );
        assert!(!partial.resolutions.is_empty());
    }

    #[test]
    fn index_is_sorted_and_deduplicated() {
        let source = "\
mncs 0.3;
module example.sorted;

fn main(input: i64) -> (result: i64) {
    let copy: i64 = input + input;
    return copy;
}";
        let index = resolutions(source);
        let mut sorted = index.resolutions.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(index.resolutions, sorted);
        assert_eq!(index.schema_version, crate::NAME_RESOLUTION_SCHEMA_VERSION);
    }

    #[test]
    fn front_end_result_exposes_the_resolution_index() {
        let envelope = SourceEnvelope::inline(
            SourceArtifactKind::Program,
            "example.frontend",
            "\
mncs 0.3;
module example.frontend;

fn helper(value: i64) -> (result: i64) {
    return value;
}

fn main(input: i64) -> (result: i64) {
    return helper(input);
}",
        );
        let front_end = crate::ReferenceCompiler::default().front_end(envelope);
        assert!(front_end.is_valid(), "{:#?}", front_end.diagnostics);
        let call = span_at(front_end.envelope.text.as_str(), "helper", 1);
        let resolved = front_end
            .name_resolutions
            .at_offset(call.start)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].kind, ResolvedNameKind::Function);
        assert_eq!(
            resolved[0].declaration,
            span_at(front_end.envelope.text.as_str(), "helper", 0)
        );
    }
}
