use std::fmt::Write;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SOURCE_ENVELOPE_SCHEMA_VERSION: &str = "0.1";
pub const SOURCE_PROFILE_VERSION: &str = "0.1";
pub const SOURCE_PROFILE_VERSION_0_2: &str = "0.2";
pub const SOURCE_PROFILE_VERSION_0_3: &str = "0.3";
pub const SOURCE_PROFILE_VERSION_0_4: &str = "0.4";
pub const SOURCE_PROFILE_VERSION_0_5: &str = "0.5";
pub const SOURCE_PROFILE_VERSION_0_6: &str = "0.6";

/// True when the active source profile declares at least `version`. Profile
/// features are strictly additive, so a numeric comparison replaces the
/// per-feature version lists that previously had to name every profile.
pub fn profile_at_least(profile: &str, version: &str) -> bool {
    let parse = |value: &str| -> Option<(u64, u64)> {
        let mut parts = value.split('.');
        let major = parts.next()?.parse::<u64>().ok()?;
        let minor = parts.next()?.parse::<u64>().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some((major, minor))
    };
    match (parse(profile), parse(version)) {
        (Some(active), Some(required)) => active >= required,
        _ => false,
    }
}
pub const LEXICAL_SCHEMA_VERSION: &str = "0.1";
pub const CST_SCHEMA_VERSION: &str = "0.1";
pub const AST_SCHEMA_VERSION: &str = "0.1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceArtifactKind {
    Program,
    Model,
    Workflow,
    VerificationSystem,
    InfrastructureDefinition,
    AutonomousComponent,
    MachineProcess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceOriginKind {
    Inline,
    Path,
    Uri,
    Generated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceOrigin {
    pub kind: SourceOriginKind,
    pub locator: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRelationshipKind {
    Contains,
    DependsOn,
    Verifies,
    Produces,
    Executes,
    Refines,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRelationship {
    pub kind: SourceRelationshipKind,
    pub target_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SourceProvenance {
    pub producer_identity: Option<String>,
    pub derived_from: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceEnvelope {
    pub schema_version: String,
    pub identity: String,
    pub language_version: String,
    pub artifact_kind: SourceArtifactKind,
    pub logical_name: String,
    pub origin: SourceOrigin,
    pub relationships: Vec<SourceRelationship>,
    pub provenance: SourceProvenance,
    pub text: String,
}

#[derive(Serialize)]
struct SourceEnvelopeMaterial<'a> {
    schema_version: &'a str,
    language_version: &'a str,
    artifact_kind: SourceArtifactKind,
    logical_name: &'a str,
    origin: &'a SourceOrigin,
    relationships: &'a [SourceRelationship],
    provenance: &'a SourceProvenance,
    text: &'a str,
}

impl SourceEnvelope {
    pub fn new(
        artifact_kind: SourceArtifactKind,
        logical_name: impl Into<String>,
        origin: SourceOrigin,
        text: impl Into<String>,
    ) -> Self {
        let text = text.into();
        let mut envelope = Self {
            schema_version: SOURCE_ENVELOPE_SCHEMA_VERSION.to_owned(),
            identity: String::new(),
            language_version: infer_source_profile(&text).to_owned(),
            artifact_kind,
            logical_name: logical_name.into(),
            origin,
            relationships: Vec::new(),
            provenance: SourceProvenance::default(),
            text,
        };
        envelope.seal();
        envelope
    }

    pub fn inline(
        artifact_kind: SourceArtifactKind,
        logical_name: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self::new(
            artifact_kind,
            logical_name,
            SourceOrigin {
                kind: SourceOriginKind::Inline,
                locator: None,
            },
            text,
        )
    }

    pub fn with_relationships(mut self, relationships: Vec<SourceRelationship>) -> Self {
        self.relationships = relationships;
        self.seal();
        self
    }

    pub fn with_provenance(mut self, provenance: SourceProvenance) -> Self {
        self.provenance = provenance;
        self.seal();
        self
    }

    pub fn seal(&mut self) {
        self.identity = format!("mncs:source:artifact:{}", fingerprint(&self.material()));
    }

    pub fn identity_is_valid(&self) -> bool {
        self.schema_version == SOURCE_ENVELOPE_SCHEMA_VERSION
            && source_profile_supported(&self.language_version)
            && !self.logical_name.trim().is_empty()
            && self
                .relationships
                .iter()
                .all(|relationship| !relationship.target_identity.trim().is_empty())
            && self
                .provenance
                .producer_identity
                .as_ref()
                .is_none_or(|identity| !identity.trim().is_empty())
            && self
                .provenance
                .derived_from
                .iter()
                .all(|identity| !identity.trim().is_empty())
            && self.identity == format!("mncs:source:artifact:{}", fingerprint(&self.material()))
    }

    fn material(&self) -> SourceEnvelopeMaterial<'_> {
        SourceEnvelopeMaterial {
            schema_version: &self.schema_version,
            language_version: &self.language_version,
            artifact_kind: self.artifact_kind,
            logical_name: &self.logical_name,
            origin: &self.origin,
            relationships: &self.relationships,
            provenance: &self.provenance,
            text: &self.text,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

impl SourceSpan {
    pub fn at(source: &str, start: usize, end: usize) -> Self {
        let prefix = &source[..start.min(source.len())];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
        let column = source[line_start..start.min(source.len())].chars().count() + 1;
        Self {
            start,
            end,
            line,
            column,
        }
    }

    fn covering(source: &str, left: Self, right: Self) -> Self {
        Self::at(source, left.start, right.end)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenKind {
    Whitespace,
    LineComment,
    BlockComment,
    MncsKeyword,
    ModuleKeyword,
    FunctionKeyword,
    ReturnKeyword,
    LetKeyword,
    IfKeyword,
    ElseKeyword,
    FailKeyword,
    RequiresKeyword,
    EnsuresKeyword,
    AssumesKeyword,
    EffectKeyword,
    CapabilityKeyword,
    AuthorizedKeyword,
    EnumKeyword,
    UseKeyword,
    RecordKeyword,
    MatchKeyword,
    IterateKeyword,
    UpToKeyword,
    CarryingKeyword,
    NextKeyword,
    WhileKeyword,
    TrueKeyword,
    FalseKeyword,
    Identifier,
    Version,
    IntegerLiteral,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    EqEq,
    NotEq,
    AndAnd,
    OrOr,
    Lt,
    Gt,
    Le,
    Ge,
    Equal,
    Dot,
    DotDot,
    Colon,
    Semicolon,
    Comma,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    Arrow,
    FatArrow,
    Unknown,
}

impl TokenKind {
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            Self::Whitespace | Self::LineComment | Self::BlockComment
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceToken {
    pub kind: TokenKind,
    pub text: String,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticStage {
    Envelope,
    Lexical,
    Parsing,
    Elaboration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceDiagnostic {
    pub code: String,
    pub stage: DiagnosticStage,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub span: SourceSpan,
    pub expected: Vec<TokenKind>,
    pub found: Option<TokenKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LexedDocument {
    pub schema_version: String,
    pub source_identity: String,
    pub tokens: Vec<SourceToken>,
    pub diagnostics: Vec<SourceDiagnostic>,
}

impl LexedDocument {
    pub fn reconstruct(&self) -> String {
        self.tokens
            .iter()
            .map(|token| token.text.as_str())
            .collect()
    }

    pub fn fingerprint(&self) -> String {
        fingerprint(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CstKind {
    Document,
    Header,
    ModuleDeclaration,
    UseDeclaration,
    FunctionDeclaration,
    FiniteTypeDeclaration,
    FiniteVariant,
    RecordTypeDeclaration,
    RecordFieldEntry,
    RecordLiteral,
    FieldProjection,
    ParameterList,
    Parameter,
    Block,
    ReturnStatement,
    BoundedIteration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CstNode {
    pub kind: CstKind,
    pub span: SourceSpan,
    pub token_start: usize,
    pub token_end: usize,
    pub children: Vec<CstNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConcreteSyntaxTree {
    pub schema_version: String,
    pub source_identity: String,
    pub source_length: usize,
    pub tokens: Vec<SourceToken>,
    pub root: CstNode,
}

impl ConcreteSyntaxTree {
    pub fn reconstruct(&self) -> String {
        self.tokens
            .iter()
            .map(|token| token.text.as_str())
            .collect()
    }

    pub fn fingerprint(&self) -> String {
        fingerprint(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpannedText {
    pub text: String,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstParameter {
    pub name: SpannedText,
    pub value_type: SpannedText,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstBody {
    pub returned_value: AstExpr,
    #[serde(default)]
    pub statements: Vec<AstStmt>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AstBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    And,
    Or,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AstExpr {
    Name(SpannedText),
    Integer {
        text: SpannedText,
        value: i128,
    },
    Boolean {
        text: SpannedText,
        value: bool,
    },
    FiniteVariant {
        type_name: SpannedText,
        variant: SpannedText,
        /// Payload fields for qualified payload construction
        /// (`Type.Variant { field: expr }`, Profile 0.6).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        fields: Vec<(SpannedText, AstExpr)>,
        span: SourceSpan,
    },
    Call {
        function: SpannedText,
        arguments: Vec<AstExpr>,
        span: SourceSpan,
    },
    Match {
        value: Box<AstExpr>,
        arms: Vec<AstMatchArm>,
        span: SourceSpan,
    },
    Binary {
        op: AstBinaryOp,
        left: Box<AstExpr>,
        right: Box<AstExpr>,
        span: SourceSpan,
    },
    RecordLiteral {
        type_name: SpannedText,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base: Option<Box<AstExpr>>,
        fields: Vec<(SpannedText, AstExpr)>,
        span: SourceSpan,
    },
    FieldProject {
        base: Box<AstExpr>,
        field: SpannedText,
        span: SourceSpan,
    },
}

impl AstExpr {
    pub fn span(&self) -> SourceSpan {
        match self {
            Self::Name(name)
            | Self::Integer { text: name, .. }
            | Self::Boolean { text: name, .. } => name.span,
            Self::FiniteVariant { span, .. }
            | Self::Call { span, .. }
            | Self::Match { span, .. }
            | Self::Binary { span, .. }
            | Self::RecordLiteral { span, .. }
            | Self::FieldProject { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstMatchArm {
    /// Optional qualifying type name (`Type.VARIANT` patterns).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_name: Option<SpannedText>,
    pub variant: SpannedText,
    /// Payload field bindings (`Variant { field: binding }` or the shorthand
    /// `Variant { field }`). Every payload field of the matched variant must
    /// be bound exactly once unless the wildcard `{ .. }` ignores the payload.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bindings: Vec<(SpannedText, SpannedText)>,
    /// Set by the `{ .. }` wildcard: the payload is carried but not observed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ignore_payload: bool,
    pub value: AstExpr,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AstStmt {
    Let {
        name: SpannedText,
        value_type: SpannedText,
        value: AstExpr,
        span: SourceSpan,
    },
    If {
        condition: AstExpr,
        then_body: Vec<AstStmt>,
        else_body: Vec<AstStmt>,
        span: SourceSpan,
    },
    Fail {
        mode: SpannedText,
        span: SourceSpan,
    },
    Return {
        value: AstExpr,
        span: SourceSpan,
    },
    BoundedIteration {
        name: SpannedText,
        bound: SpannedText,
        bound_value: i128,
        state: SpannedText,
        state_type: SpannedText,
        initial: Box<AstExpr>,
        body: Vec<AstStmt>,
        next_state: SpannedText,
        next_value: Box<AstExpr>,
        span: SourceSpan,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstClause {
    pub kind: SpannedText,
    pub name: SpannedText,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstEffect {
    pub kind: SpannedText,
    pub capability: SpannedText,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstFunction {
    pub name: SpannedText,
    pub inputs: Vec<AstParameter>,
    pub outputs: Vec<AstParameter>,
    #[serde(default)]
    pub contracts: Vec<AstClause>,
    #[serde(default)]
    pub effects: Vec<AstEffect>,
    #[serde(default)]
    pub capabilities: Vec<SpannedText>,
    pub body: AstBody,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstFiniteVariant {
    pub name: SpannedText,
    /// Payload fields (Profile 0.6). Empty for payload-free variants; the
    /// serialized form of payload-free variants is unchanged.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<AstRecordField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstFiniteType {
    pub name: SpannedText,
    pub variants: Vec<AstFiniteVariant>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstRecordField {
    pub name: SpannedText,
    pub value_type: SpannedText,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstRecordDecl {
    pub name: SpannedText,
    pub fields: Vec<AstRecordField>,
    pub span: SourceSpan,
}

/// A module import declaration (`use other.module;`). The named module is a
/// semantic namespace whose exported declarations bind into this module's
/// scope; compatibility is established by elaboration of the named module,
/// never by the name itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstUseDecl {
    pub module: SpannedText,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbstractSyntaxTree {
    pub schema_version: String,
    pub source_identity: String,
    pub language_version: SpannedText,
    pub module: SpannedText,
    #[serde(default)]
    pub uses: Vec<AstUseDecl>,
    #[serde(default)]
    pub finite_types: Vec<AstFiniteType>,
    #[serde(default)]
    pub record_types: Vec<AstRecordDecl>,
    pub functions: Vec<AstFunction>,
    pub span: SourceSpan,
}

impl AbstractSyntaxTree {
    pub fn fingerprint(&self) -> String {
        fingerprint(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseOutput {
    pub lexical: LexedDocument,
    pub cst: ConcreteSyntaxTree,
    pub ast: Option<AbstractSyntaxTree>,
    pub diagnostics: Vec<SourceDiagnostic>,
}

impl ParseOutput {
    pub fn is_valid(&self) -> bool {
        self.ast.is_some()
            && self
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.severity != DiagnosticSeverity::Error)
    }
}

pub fn lex(envelope: &SourceEnvelope) -> LexedDocument {
    let source = envelope.text.as_str();
    let mut tokens = Vec::new();
    let mut diagnostics = Vec::new();
    let mut offset = 0;
    while offset < source.len() {
        let current = source[offset..]
            .chars()
            .next()
            .expect("offset is within source");
        let start = offset;
        let kind = if current.is_whitespace() {
            offset += current.len_utf8();
            while offset < source.len()
                && source[offset..]
                    .chars()
                    .next()
                    .is_some_and(char::is_whitespace)
            {
                offset += source[offset..]
                    .chars()
                    .next()
                    .expect("offset is within source")
                    .len_utf8();
            }
            TokenKind::Whitespace
        } else if source[start..].starts_with("//") {
            offset += 2;
            while offset < source.len() && !source[offset..].starts_with('\n') {
                offset += source[offset..]
                    .chars()
                    .next()
                    .expect("offset is within source")
                    .len_utf8();
            }
            TokenKind::LineComment
        } else if source[start..].starts_with("/*") {
            offset += 2;
            let mut depth = 1usize;
            while offset < source.len() && depth > 0 {
                if source[offset..].starts_with("/*") {
                    depth += 1;
                    offset += 2;
                } else if source[offset..].starts_with("*/") {
                    depth -= 1;
                    offset += 2;
                } else {
                    offset += source[offset..]
                        .chars()
                        .next()
                        .expect("offset is within source")
                        .len_utf8();
                }
            }
            if depth > 0 {
                diagnostics.push(SourceDiagnostic {
                    code: "MNL001".to_owned(),
                    stage: DiagnosticStage::Lexical,
                    severity: DiagnosticSeverity::Error,
                    message: "unterminated block comment".to_owned(),
                    span: SourceSpan::at(source, start, offset),
                    expected: Vec::new(),
                    found: None,
                });
            }
            TokenKind::BlockComment
        } else if source[start..].starts_with("->") {
            offset += 2;
            TokenKind::Arrow
        } else if source[start..].starts_with("=>") {
            offset += 2;
            TokenKind::FatArrow
        } else if source[start..].starts_with("&&") {
            offset += 2;
            TokenKind::AndAnd
        } else if source[start..].starts_with("||") {
            offset += 2;
            TokenKind::OrOr
        } else if source[start..].starts_with("==") {
            offset += 2;
            TokenKind::EqEq
        } else if source[start..].starts_with("!=") {
            offset += 2;
            TokenKind::NotEq
        } else if source[start..].starts_with("<=") {
            offset += 2;
            TokenKind::Le
        } else if source[start..].starts_with(">=") {
            offset += 2;
            TokenKind::Ge
        } else if is_identifier_start(current) {
            offset += current.len_utf8();
            while offset < source.len()
                && source[offset..]
                    .chars()
                    .next()
                    .is_some_and(is_identifier_continue)
            {
                offset += source[offset..]
                    .chars()
                    .next()
                    .expect("offset is within source")
                    .len_utf8();
            }
            match &source[start..offset] {
                "mncs" => TokenKind::MncsKeyword,
                "module" => TokenKind::ModuleKeyword,
                "fn" => TokenKind::FunctionKeyword,
                "return" => TokenKind::ReturnKeyword,
                "let" => TokenKind::LetKeyword,
                "if" => TokenKind::IfKeyword,
                "else" => TokenKind::ElseKeyword,
                "fail" => TokenKind::FailKeyword,
                "requires" => TokenKind::RequiresKeyword,
                "ensures" => TokenKind::EnsuresKeyword,
                "assumes" => TokenKind::AssumesKeyword,
                "effect" => TokenKind::EffectKeyword,
                "capability" => TokenKind::CapabilityKeyword,
                "authorized_by" => TokenKind::AuthorizedKeyword,
                "enum" => TokenKind::EnumKeyword,
                "use" => TokenKind::UseKeyword,
                "record" => TokenKind::RecordKeyword,
                "match" => TokenKind::MatchKeyword,
                "iterate" => TokenKind::IterateKeyword,
                "up_to" => TokenKind::UpToKeyword,
                "carrying" => TokenKind::CarryingKeyword,
                "next" => TokenKind::NextKeyword,
                "while" => TokenKind::WhileKeyword,
                "true" => TokenKind::TrueKeyword,
                "false" => TokenKind::FalseKeyword,
                _ => TokenKind::Identifier,
            }
        } else if current.is_ascii_digit() {
            offset += current.len_utf8();
            let mut dotted = false;
            while offset < source.len()
                && source[offset..]
                    .chars()
                    .next()
                    .is_some_and(|value| value.is_ascii_digit() || value == '.')
            {
                if source[offset..].starts_with('.') {
                    dotted = true;
                }
                offset += 1;
            }
            if dotted {
                TokenKind::Version
            } else {
                TokenKind::IntegerLiteral
            }
        } else {
            offset += current.len_utf8();
            match current {
                '.' if source[offset..].starts_with('.') => {
                    offset += 1;
                    TokenKind::DotDot
                }
                '.' => TokenKind::Dot,
                ':' => TokenKind::Colon,
                ';' => TokenKind::Semicolon,
                ',' => TokenKind::Comma,
                '(' => TokenKind::LeftParen,
                ')' => TokenKind::RightParen,
                '{' => TokenKind::LeftBrace,
                '}' => TokenKind::RightBrace,
                '+' => TokenKind::Plus,
                '-' => TokenKind::Minus,
                '*' => TokenKind::Star,
                '/' => TokenKind::Slash,
                '%' => TokenKind::Percent,
                '<' => TokenKind::Lt,
                '>' => TokenKind::Gt,
                '=' => TokenKind::Equal,
                _ => {
                    diagnostics.push(SourceDiagnostic {
                        code: "MNL002".to_owned(),
                        stage: DiagnosticStage::Lexical,
                        severity: DiagnosticSeverity::Error,
                        message: format!("unsupported source character {current:?}"),
                        span: SourceSpan::at(source, start, offset),
                        expected: Vec::new(),
                        found: Some(TokenKind::Unknown),
                    });
                    TokenKind::Unknown
                }
            }
        };
        tokens.push(SourceToken {
            kind,
            text: source[start..offset].to_owned(),
            span: SourceSpan::at(source, start, offset),
        });
    }
    LexedDocument {
        schema_version: LEXICAL_SCHEMA_VERSION.to_owned(),
        source_identity: envelope.identity.clone(),
        tokens,
        diagnostics,
    }
}

pub fn parse(envelope: &SourceEnvelope) -> ParseOutput {
    let lexical = lex(envelope);
    let mut parser = Parser::new(envelope, &lexical.tokens);
    let (cst, ast) = parser.document();
    let mut diagnostics = lexical.diagnostics.clone();
    diagnostics.append(&mut parser.diagnostics);
    if !envelope.identity_is_valid() {
        diagnostics.push(SourceDiagnostic {
            code: "MNE001".to_owned(),
            stage: DiagnosticStage::Envelope,
            severity: DiagnosticSeverity::Error,
            message: "source envelope identity or schema is invalid".to_owned(),
            span: SourceSpan::at(&envelope.text, 0, 0),
            expected: Vec::new(),
            found: None,
        });
    }
    if ast
        .as_ref()
        .is_some_and(|tree| tree.language_version.text != envelope.language_version)
    {
        diagnostics.push(SourceDiagnostic {
            code: "MNE002".to_owned(),
            stage: DiagnosticStage::Envelope,
            severity: DiagnosticSeverity::Error,
            message: "source header version does not match its envelope".to_owned(),
            span: ast.as_ref().expect("checked above").language_version.span,
            expected: Vec::new(),
            found: Some(TokenKind::Version),
        });
    }
    let ast = diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity != DiagnosticSeverity::Error)
        .then_some(ast)
        .flatten();
    ParseOutput {
        lexical,
        cst,
        ast,
        diagnostics,
    }
}

struct Parser<'a> {
    envelope: &'a SourceEnvelope,
    tokens: &'a [SourceToken],
    significant: Vec<usize>,
    cursor: usize,
    diagnostics: Vec<SourceDiagnostic>,
    profile: String,
}

impl<'a> Parser<'a> {
    fn new(envelope: &'a SourceEnvelope, tokens: &'a [SourceToken]) -> Self {
        Self {
            envelope,
            tokens,
            significant: tokens
                .iter()
                .enumerate()
                .filter_map(|(index, token)| (!token.kind.is_trivia()).then_some(index))
                .collect(),
            cursor: 0,
            diagnostics: Vec::new(),
            profile: SOURCE_PROFILE_VERSION.to_owned(),
        }
    }

    fn document(&mut self) -> (ConcreteSyntaxTree, Option<AbstractSyntaxTree>) {
        let header_start = self.current_token_index();
        let header_keyword = self.expect(TokenKind::MncsKeyword, "MNP001", "expected MNCS header");
        let version = self.spanned(TokenKind::Version, "MNP002", "expected language version");
        if let Some(version) = &version {
            self.profile.clone_from(&version.text);
        }
        self.expect(
            TokenKind::Semicolon,
            "MNP003",
            "expected ';' after language version",
        );
        let header_end = self.previous_token_index(header_start);
        let header = self.node(CstKind::Header, header_start, header_end, Vec::new());

        let module_start = self.current_token_index();
        self.expect(
            TokenKind::ModuleKeyword,
            "MNP004",
            "expected module declaration",
        );
        let module = self.qualified_name();
        self.expect(
            TokenKind::Semicolon,
            "MNP005",
            "expected ';' after module name",
        );
        let module_end = self.previous_token_index(module_start);
        let module_node = self.node(
            CstKind::ModuleDeclaration,
            module_start,
            module_end,
            Vec::new(),
        );

        // Module imports precede every other declaration.
        let mut uses = Vec::new();
        let mut use_nodes = Vec::new();
        while self.current_kind() == Some(TokenKind::UseKeyword) {
            let (node, use_decl) = self.use_decl();
            use_nodes.push(node);
            if let Some(use_decl) = use_decl {
                uses.push(use_decl);
            }
        }
        // Declarations may be interleaved freely: enums, records, and
        // functions may appear in any order. Each construct remains gated by
        // its own source profile.
        let mut finite_types = Vec::new();
        let mut record_types = Vec::new();
        let mut functions = Vec::new();
        let mut declaration_nodes = Vec::new();
        loop {
            match self.current_kind() {
                Some(TokenKind::EnumKeyword)
                    if profile_at_least(&self.profile, SOURCE_PROFILE_VERSION_0_3) =>
                {
                    let (node, finite_type) = self.finite_type();
                    declaration_nodes.push(node);
                    if let Some(finite_type) = finite_type {
                        finite_types.push(finite_type);
                    }
                }
                Some(TokenKind::RecordKeyword) => {
                    if !profile_at_least(&self.profile, SOURCE_PROFILE_VERSION_0_5) {
                        self.error(
                            "MNP120",
                            "record declarations require source profile 0.5 or later",
                            vec![TokenKind::RecordKeyword],
                        );
                        break;
                    }
                    let (node, record_decl) = self.record_decl();
                    declaration_nodes.push(node);
                    if let Some(record_decl) = record_decl {
                        record_types.push(record_decl);
                    }
                }
                Some(TokenKind::FunctionKeyword) => {
                    let (node, function) = self.function();
                    declaration_nodes.push(node);
                    if let Some(function) = function {
                        functions.push(function);
                    }
                }
                _ => break,
            }
        }
        // Outside Profile 0.5 `record` is an ordinary identifier; a stray
        // declaration still deserves the precise diagnostic, not a generic
        // parse failure.
        if self.current_kind() == Some(TokenKind::Identifier)
            && self
                .current_token()
                .is_some_and(|token| token.text == "record")
        {
            self.error(
                "MNP120",
                "record declarations require source profile 0.5 or later",
                vec![TokenKind::Identifier],
            );
            self.cursor += 1;
        }
        // A module must declare something: a function, a type, or an import.
        // Vocabulary-only modules (shared enums/records) are legitimate.
        let declared_anything = !(functions.is_empty()
            && finite_types.is_empty()
            && record_types.is_empty()
            && uses.is_empty());
        if !declared_anything {
            self.error(
                "MNP006",
                "module declares nothing; expected at least one function, type, or import",
                vec![TokenKind::FunctionKeyword],
            );
        }
        if self.cursor < self.significant.len() {
            self.error(
                "MNP007",
                "unexpected token after the final function",
                Vec::new(),
            );
        }

        let source_span = SourceSpan::at(&self.envelope.text, 0, self.envelope.text.len());
        let mut children = vec![header, module_node];
        children.extend(use_nodes);
        children.extend(declaration_nodes);
        let root = CstNode {
            kind: CstKind::Document,
            span: source_span,
            token_start: 0,
            token_end: self.tokens.len(),
            children,
        };
        let cst = ConcreteSyntaxTree {
            schema_version: CST_SCHEMA_VERSION.to_owned(),
            source_identity: self.envelope.identity.clone(),
            source_length: self.envelope.text.len(),
            tokens: self.tokens.to_vec(),
            root,
        };
        let ast = match (header_keyword, version, module) {
            (Some(_), Some(language_version), Some(module)) if declared_anything => {
                Some(AbstractSyntaxTree {
                    schema_version: AST_SCHEMA_VERSION.to_owned(),
                    source_identity: self.envelope.identity.clone(),
                    language_version,
                    module,
                    uses,
                    finite_types,
                    record_types,
                    functions,
                    span: source_span,
                })
            }
            _ => None,
        };
        (cst, ast)
    }

    /// Parses `use other.module;`. Module imports require Profile 0.6; in
    /// earlier profiles the `use` keyword is an ordinary identifier, so the
    /// declaration shape produces a precise diagnostic rather than silence.
    fn use_decl(&mut self) -> (CstNode, Option<AstUseDecl>) {
        let start = self.current_token_index();
        self.expect(TokenKind::UseKeyword, "MNP135", "expected 'use'");
        if !profile_at_least(&self.profile, SOURCE_PROFILE_VERSION_0_6) {
            self.error(
                "MNP136",
                "module imports require source profile 0.6 or later",
                vec![TokenKind::UseKeyword],
            );
            return (
                self.node(CstKind::UseDeclaration, start, start, Vec::new()),
                None,
            );
        }
        let module = self.qualified_name();
        self.expect(
            TokenKind::Semicolon,
            "MNP137",
            "expected ';' after imported module name",
        );
        let end = self.previous_token_index(start);
        let node = self.node(CstKind::UseDeclaration, start, end, Vec::new());
        let span = match &module {
            Some(module) => SourceSpan::covering(&self.envelope.text, module.span, module.span),
            None => self
                .tokens
                .get(*self.significant.get(start).unwrap_or(&0))
                .map_or(SourceSpan::at(&self.envelope.text, 0, 0), |token| {
                    token.span
                }),
        };
        let decl = module.map(|module| AstUseDecl { module, span });
        (node, decl)
    }

    fn finite_type(&mut self) -> (CstNode, Option<AstFiniteType>) {
        let start = self.current_token_index();
        self.expect(TokenKind::EnumKeyword, "MNP070", "expected 'enum'");
        let name = self.spanned(TokenKind::Identifier, "MNP071", "expected finite type name");
        self.expect(
            TokenKind::LeftBrace,
            "MNP072",
            "expected '{' after finite type name",
        );
        let mut variants = Vec::new();
        let mut children = Vec::new();
        while self.current_kind() == Some(TokenKind::Identifier) {
            let variant_start = self.current_token_index();
            let mut variant_fields = Vec::new();
            if let Some(variant) =
                self.spanned(TokenKind::Identifier, "MNP073", "expected variant name")
            {
                if self.current_kind() == Some(TokenKind::LeftBrace)
                    && matches!(self.profile.as_str(), SOURCE_PROFILE_VERSION_0_6)
                {
                    // Payload variant: `Name { field: Type, ... }` with the
                    // same field syntax as record declarations.
                    self.cursor += 1;
                    while let Some(field_name) = self.spanned(
                        TokenKind::Identifier,
                        "MNP132",
                        "expected payload field name",
                    ) {
                        self.expect(
                            TokenKind::Colon,
                            "MNP133",
                            "expected ':' after payload field name",
                        );
                        let Some(field_type) = self.spanned(
                            TokenKind::Identifier,
                            "MNP134",
                            "expected payload field type",
                        ) else {
                            break;
                        };
                        let field_span = SourceSpan::covering(
                            &self.envelope.text,
                            field_name.span,
                            field_type.span,
                        );
                        variant_fields.push(AstRecordField {
                            name: field_name,
                            value_type: field_type,
                            span: field_span,
                        });
                        if self.current_kind() != Some(TokenKind::Comma) {
                            break;
                        }
                        self.cursor += 1;
                    }
                    self.expect(
                        TokenKind::RightBrace,
                        "MNP135",
                        "expected '}' after payload fields",
                    );
                } else if self.current_kind() == Some(TokenKind::LeftBrace) {
                    self.error(
                        "MNP076",
                        "payload variant fields require source profile 0.6",
                        vec![TokenKind::RightBrace],
                    );
                }
                variants.push(AstFiniteVariant {
                    name: variant,
                    fields: variant_fields,
                });
            }
            let variant_end = self.previous_token_index(variant_start);
            children.push(self.node(
                CstKind::FiniteVariant,
                variant_start,
                variant_end,
                Vec::new(),
            ));
            if self.current_kind() != Some(TokenKind::Comma) {
                break;
            }
            self.cursor += 1;
        }
        self.expect(
            TokenKind::RightBrace,
            "MNP074",
            "expected '}' after finite variants",
        );
        let end = self.previous_token_index(start);
        let node = self.node(CstKind::FiniteTypeDeclaration, start, end, children);
        if variants.is_empty() {
            self.error(
                "MNP075",
                "finite type requires at least one variant",
                vec![TokenKind::Identifier],
            );
        }
        let finite_type = name.map(|name| AstFiniteType {
            name,
            variants,
            span: node.span,
        });
        (node, finite_type)
    }

    fn record_decl(&mut self) -> (CstNode, Option<AstRecordDecl>) {
        let start = self.current_token_index();
        self.expect(TokenKind::RecordKeyword, "MNP121", "expected 'record'");
        let name = self.spanned(TokenKind::Identifier, "MNP122", "expected record name");
        self.expect(
            TokenKind::LeftBrace,
            "MNP123",
            "expected '{' after record name",
        );
        let mut fields: Vec<AstRecordField> = Vec::new();
        let mut children = Vec::new();
        while self.current_kind() == Some(TokenKind::Identifier) {
            let field_start = self.current_token_index();
            let field_name = self.spanned(TokenKind::Identifier, "MNP124", "expected field name");
            self.expect(TokenKind::Colon, "MNP125", "expected ':' after field name");
            let field_type = self.spanned(TokenKind::Identifier, "MNP126", "expected field type");
            let field_end = self.previous_token_index(field_start);
            if let (Some(field_name), Some(field_type)) = (field_name.clone(), field_type.clone()) {
                let field_span =
                    SourceSpan::covering(&self.envelope.text, field_name.span, field_type.span);
                fields.push(AstRecordField {
                    name: field_name,
                    value_type: field_type,
                    span: field_span,
                });
            }
            children.push(self.node(
                CstKind::RecordFieldEntry,
                field_start,
                field_end,
                Vec::new(),
            ));
            if self.current_kind() != Some(TokenKind::Comma) {
                break;
            }
            self.cursor += 1;
        }
        self.expect(
            TokenKind::RightBrace,
            "MNP127",
            "expected '}' after record fields",
        );
        let end = self.previous_token_index(start);
        let node = self.node(CstKind::RecordTypeDeclaration, start, end, children);
        if fields.is_empty() {
            self.error(
                "MNP128",
                "record requires at least one field",
                vec![TokenKind::Identifier],
            );
        }
        let record = name.map(|name| AstRecordDecl {
            name,
            fields,
            span: node.span,
        });
        (node, record)
    }

    fn function(&mut self) -> (CstNode, Option<AstFunction>) {
        let start = self.current_token_index();
        self.expect(TokenKind::FunctionKeyword, "MNP010", "expected 'fn'");
        let name = self.spanned(TokenKind::Identifier, "MNP011", "expected function name");
        let (input_node, inputs) = self.parameter_list("input");
        self.expect(TokenKind::Arrow, "MNP012", "expected '->' before outputs");
        let (output_node, outputs) = self.parameter_list("output");
        let (contracts, effects, capabilities) =
            if profile_at_least(&self.profile, SOURCE_PROFILE_VERSION_0_2) {
                self.clauses()
            } else {
                (Vec::new(), Vec::new(), Vec::new())
            };
        let block_start = self.current_token_index();
        self.expect(TokenKind::LeftBrace, "MNP013", "expected function body");
        let (statements, returned_value, block_children) =
            if profile_at_least(&self.profile, SOURCE_PROFILE_VERSION_0_2) {
                self.statements_until_return()
            } else {
                let return_start = self.current_token_index();
                self.expect(
                    TokenKind::ReturnKeyword,
                    "MNP014",
                    "expected explicit return",
                );
                let returned_value = self
                    .spanned(
                        TokenKind::Identifier,
                        "MNP015",
                        "expected returned value name",
                    )
                    .map(AstExpr::Name);
                self.expect(TokenKind::Semicolon, "MNP016", "expected ';' after return");
                let return_end = self.previous_token_index(return_start);
                (
                    Vec::new(),
                    returned_value,
                    vec![self.node(
                        CstKind::ReturnStatement,
                        return_start,
                        return_end,
                        Vec::new(),
                    )],
                )
            };
        self.expect(
            TokenKind::RightBrace,
            "MNP017",
            "expected '}' after function body",
        );
        let end = self.previous_token_index(start);
        let block_node = self.node(CstKind::Block, block_start, end, block_children);
        let body_span = block_node.span;
        let function_node = self.node(
            CstKind::FunctionDeclaration,
            start,
            end,
            vec![input_node, output_node, block_node],
        );
        let function = match (name, returned_value) {
            (Some(name), Some(returned_value)) => Some(AstFunction {
                name,
                inputs,
                outputs,
                contracts,
                effects,
                capabilities,
                body: AstBody {
                    returned_value,
                    statements,
                    span: body_span,
                },
                span: function_node.span,
            }),
            _ => None,
        };
        (function_node, function)
    }

    fn clauses(&mut self) -> (Vec<AstClause>, Vec<AstEffect>, Vec<SpannedText>) {
        let mut contracts = Vec::new();
        let mut effects = Vec::new();
        let mut capabilities = Vec::new();
        loop {
            match self.current_kind() {
                Some(
                    TokenKind::RequiresKeyword
                    | TokenKind::EnsuresKeyword
                    | TokenKind::AssumesKeyword,
                ) => {
                    let start = self.current_token_index();
                    let kind =
                        self.spanned(self.current_kind().unwrap(), "MNP040", "expected clause");
                    let name =
                        self.spanned(TokenKind::Identifier, "MNP041", "expected clause name");
                    if let (Some(kind), Some(name)) = (kind, name) {
                        let span = SourceSpan::covering(&self.envelope.text, kind.span, name.span);
                        contracts.push(AstClause { kind, name, span });
                    }
                    let _ = start;
                }
                Some(TokenKind::CapabilityKeyword) => {
                    self.cursor += 1;
                    if let Some(name) =
                        self.spanned(TokenKind::Identifier, "MNP042", "expected capability name")
                    {
                        capabilities.push(name);
                    }
                }
                Some(TokenKind::EffectKeyword) => {
                    let start = self.current_token_index();
                    self.cursor += 1;
                    let kind =
                        self.spanned(TokenKind::Identifier, "MNP043", "expected effect kind");
                    self.expect(
                        TokenKind::AuthorizedKeyword,
                        "MNP044",
                        "expected 'authorized_by' after effect",
                    );
                    let capability = self.spanned(
                        TokenKind::Identifier,
                        "MNP045",
                        "expected authorizing capability",
                    );
                    if let (Some(kind), Some(capability)) = (kind, capability) {
                        effects.push(AstEffect {
                            span: SourceSpan::covering(
                                &self.envelope.text,
                                kind.span,
                                capability.span,
                            ),
                            kind,
                            capability,
                        });
                    }
                    let _ = start;
                }
                _ => break,
            }
        }
        (contracts, effects, capabilities)
    }

    fn statements_until_return(&mut self) -> (Vec<AstStmt>, Option<AstExpr>, Vec<CstNode>) {
        let mut statements = Vec::new();
        let mut children = Vec::new();
        let mut returned = None;
        while self.current_kind() != Some(TokenKind::RightBrace)
            && self.current_kind() != Some(TokenKind::ReturnKeyword)
            && self.cursor < self.significant.len()
        {
            let (node, stmt) = self.statement();
            children.push(node);
            if let Some(stmt) = stmt {
                statements.push(stmt);
            }
        }
        if self.current_kind() == Some(TokenKind::ReturnKeyword) {
            let return_start = self.current_token_index();
            self.cursor += 1;
            returned = if profile_at_least(&self.profile, SOURCE_PROFILE_VERSION_0_3) {
                self.expression()
            } else {
                self.spanned(
                    TokenKind::Identifier,
                    "MNP015",
                    "expected returned value name",
                )
                .map(AstExpr::Name)
            };
            self.expect(TokenKind::Semicolon, "MNP016", "expected ';' after return");
            let return_end = self.previous_token_index(return_start);
            children.push(self.node(
                CstKind::ReturnStatement,
                return_start,
                return_end,
                Vec::new(),
            ));
        } else {
            self.error(
                "MNP014",
                "expected explicit return",
                vec![TokenKind::ReturnKeyword],
            );
        }
        (statements, returned, children)
    }

    fn statement(&mut self) -> (CstNode, Option<AstStmt>) {
        let start = self.current_token_index();
        match self.current_kind() {
            Some(TokenKind::LetKeyword) => {
                self.cursor += 1;
                let name = self.spanned(TokenKind::Identifier, "MNP050", "expected binding name");
                self.expect(
                    TokenKind::Colon,
                    "MNP051",
                    "expected ':' after binding name",
                );
                let value_type =
                    self.spanned(TokenKind::Identifier, "MNP052", "expected binding type");
                // '=' is colon-like; accept identifier after a dummy skip if '=' unknown
                self.expect(
                    TokenKind::Equal,
                    "MNP053",
                    "expected '=' after binding type",
                );
                let value = self.expression();
                self.expect(TokenKind::Semicolon, "MNP054", "expected ';' after let");
                let end = self.previous_token_index(start);
                let node = self.node(CstKind::Block, start, end, Vec::new());
                let stmt = match (name, value_type, value) {
                    (Some(name), Some(value_type), Some(value)) => Some(AstStmt::Let {
                        span: SourceSpan::covering(&self.envelope.text, name.span, value.span()),
                        name,
                        value_type,
                        value,
                    }),
                    _ => None,
                };
                (node, stmt)
            }
            Some(TokenKind::IfKeyword) => {
                self.cursor += 1;
                let condition = self.expression();
                self.expect(TokenKind::LeftBrace, "MNP055", "expected '{' after if");
                let mut then_body = Vec::new();
                while self.current_kind() != Some(TokenKind::RightBrace)
                    && self.cursor < self.significant.len()
                {
                    let (_, stmt) = self.statement();
                    if let Some(stmt) = stmt {
                        then_body.push(stmt);
                    }
                }
                self.expect(
                    TokenKind::RightBrace,
                    "MNP056",
                    "expected '}' after if body",
                );
                let mut else_body = Vec::new();
                if self.current_kind() == Some(TokenKind::ElseKeyword) {
                    self.cursor += 1;
                    self.expect(TokenKind::LeftBrace, "MNP057", "expected '{' after else");
                    while self.current_kind() != Some(TokenKind::RightBrace)
                        && self.cursor < self.significant.len()
                    {
                        let (_, stmt) = self.statement();
                        if let Some(stmt) = stmt {
                            else_body.push(stmt);
                        }
                    }
                    self.expect(TokenKind::RightBrace, "MNP058", "expected '}' after else");
                }
                let end = self.previous_token_index(start);
                let node = self.node(CstKind::Block, start, end, Vec::new());
                (
                    node,
                    condition.map(|condition| AstStmt::If {
                        span: condition.span(),
                        condition,
                        then_body,
                        else_body,
                    }),
                )
            }
            Some(TokenKind::FailKeyword) => {
                self.cursor += 1;
                let mode = self.spanned(TokenKind::Identifier, "MNP059", "expected failure mode");
                self.expect(TokenKind::Semicolon, "MNP060", "expected ';' after fail");
                let end = self.previous_token_index(start);
                let node = self.node(CstKind::Block, start, end, Vec::new());
                (
                    node,
                    mode.map(|mode| AstStmt::Fail {
                        span: mode.span,
                        mode,
                    }),
                )
            }
            Some(TokenKind::ReturnKeyword) => {
                self.cursor += 1;
                let value = if profile_at_least(&self.profile, SOURCE_PROFILE_VERSION_0_3) {
                    self.expression()
                } else {
                    self.spanned(
                        TokenKind::Identifier,
                        "MNP015",
                        "expected returned value name",
                    )
                    .map(AstExpr::Name)
                };
                self.expect(TokenKind::Semicolon, "MNP016", "expected ';' after return");
                let end = self.previous_token_index(start);
                let node = self.node(CstKind::ReturnStatement, start, end, Vec::new());
                (
                    node,
                    value.map(|value| AstStmt::Return {
                        span: value.span(),
                        value,
                    }),
                )
            }
            Some(TokenKind::IterateKeyword) => self.bounded_iteration_statement(start),
            Some(TokenKind::WhileKeyword) => {
                self.error(
                    "MNP106",
                    "unbounded 'while' iteration is unsupported; Profile 0.4 requires 'iterate ... up_to <literal>'",
                    vec![TokenKind::IterateKeyword],
                );
                self.cursor += 1;
                let end = self.previous_token_index(start);
                (self.node(CstKind::Block, start, end, Vec::new()), None)
            }
            _ => {
                self.error(
                    "MNP061",
                    "expected let, if, fail, return, or bounded iteration",
                    vec![
                        TokenKind::LetKeyword,
                        TokenKind::IfKeyword,
                        TokenKind::FailKeyword,
                        TokenKind::ReturnKeyword,
                        TokenKind::IterateKeyword,
                    ],
                );
                if self.cursor < self.significant.len() {
                    self.cursor += 1;
                }
                let end = self.previous_token_index(start);
                (self.node(CstKind::Block, start, end, Vec::new()), None)
            }
        }
    }

    fn bounded_iteration_statement(&mut self, start: usize) -> (CstNode, Option<AstStmt>) {
        if !profile_at_least(&self.profile, SOURCE_PROFILE_VERSION_0_4) {
            self.error(
                "MNP090",
                "bounded iteration requires Source Profile 0.4 or later",
                Vec::new(),
            );
        }
        self.expect(TokenKind::IterateKeyword, "MNP091", "expected 'iterate'");
        let name = self.spanned(
            TokenKind::Identifier,
            "MNP092",
            "expected iteration identity",
        );
        self.expect(TokenKind::UpToKeyword, "MNP093", "expected 'up_to'");
        let bound = self.spanned(
            TokenKind::IntegerLiteral,
            "MNP094",
            "expected a literal iteration bound",
        );
        let bound_value = bound
            .as_ref()
            .and_then(|bound| bound.text.parse::<i128>().ok());
        self.expect(
            TokenKind::CarryingKeyword,
            "MNP095",
            "expected 'carrying' after iteration bound",
        );
        let state = self.spanned(
            TokenKind::Identifier,
            "MNP096",
            "expected carried state name",
        );
        self.expect(
            TokenKind::Colon,
            "MNP097",
            "expected ':' after carried state",
        );
        let state_type = self.spanned(
            TokenKind::Identifier,
            "MNP098",
            "expected carried state type",
        );
        self.expect(
            TokenKind::Equal,
            "MNP099",
            "expected '=' before initial carried state",
        );
        let initial = self.expression();
        self.expect(
            TokenKind::LeftBrace,
            "MNP100",
            "expected '{' before iteration body",
        );
        let mut body = Vec::new();
        while !matches!(
            self.current_kind(),
            Some(TokenKind::NextKeyword | TokenKind::RightBrace) | None
        ) {
            let (_, statement) = self.statement();
            if let Some(statement) = statement {
                body.push(statement);
            }
        }
        self.expect(
            TokenKind::NextKeyword,
            "MNP101",
            "iteration body must end with an explicit 'next' transition",
        );
        let next_state = self.spanned(
            TokenKind::Identifier,
            "MNP102",
            "expected carried state after 'next'",
        );
        self.expect(
            TokenKind::Equal,
            "MNP103",
            "expected '=' in iteration state transition",
        );
        let next_value = self.expression();
        self.expect(
            TokenKind::Semicolon,
            "MNP104",
            "expected ';' after iteration state transition",
        );
        self.expect(
            TokenKind::RightBrace,
            "MNP105",
            "expected '}' after iteration body",
        );
        let end = self.previous_token_index(start);
        let node = self.node(CstKind::BoundedIteration, start, end, Vec::new());
        let statement = match (
            name,
            bound,
            bound_value,
            state,
            state_type,
            initial,
            next_state,
            next_value,
        ) {
            (
                Some(name),
                Some(bound),
                Some(bound_value),
                Some(state),
                Some(state_type),
                Some(initial),
                Some(next_state),
                Some(next_value),
            ) => Some(AstStmt::BoundedIteration {
                name,
                bound,
                bound_value,
                state,
                state_type,
                initial: Box::new(initial),
                body,
                next_state,
                next_value: Box::new(next_value),
                span: node.span,
            }),
            _ => None,
        };
        (node, statement)
    }

    fn expression(&mut self) -> Option<AstExpr> {
        self.binary_expression(0)
    }

    fn binary_expression(&mut self, minimum_precedence: u8) -> Option<AstExpr> {
        let mut left = self.primary()?;
        while let Some((op, precedence)) = binary_operator(self.current_kind()) {
            if precedence < minimum_precedence {
                break;
            }
            self.cursor += 1;
            let right = self.binary_expression(precedence + 1)?;
            let span = SourceSpan::covering(&self.envelope.text, left.span(), right.span());
            left = AstExpr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Some(left)
    }

    fn primary(&mut self) -> Option<AstExpr> {
        match self.current_kind() {
            Some(TokenKind::Identifier) => {
                let name = self.spanned(TokenKind::Identifier, "MNP062", "expected expression")?;
                if self.current_kind() == Some(TokenKind::LeftBrace)
                    && profile_at_least(&self.profile, SOURCE_PROFILE_VERSION_0_5)
                    && self.at_record_literal()
                {
                    return self.record_literal(name);
                }
                let primary_expr = if self.current_kind() == Some(TokenKind::Dot) {
                    self.cursor += 1;
                    let variant = self.spanned(
                        TokenKind::Identifier,
                        "MNP065",
                        "expected finite variant or field after '.'",
                    )?;
                    let span = SourceSpan::covering(&self.envelope.text, name.span, variant.span);
                    // Qualified payload construction: `Type.Variant { ... }`
                    // (Profile 0.6). The lookahead distinguishes it from a
                    // projection chain by checking for a literal-opening '{'.
                    if self.at_payload_construct()
                        && matches!(self.profile.as_str(), SOURCE_PROFILE_VERSION_0_6)
                    {
                        self.cursor += 1;
                        let mut fields = Vec::new();
                        while self.current_kind() != Some(TokenKind::RightBrace)
                            && self.cursor < self.significant.len()
                        {
                            let Some(field_name) = self.spanned(
                                TokenKind::Identifier,
                                "MNP140",
                                "expected payload field name",
                            ) else {
                                break;
                            };
                            self.expect(
                                TokenKind::Colon,
                                "MNP141",
                                "expected ':' after payload field name",
                            );
                            let field_value = self.expression()?;
                            fields.push((field_name, field_value));
                            if self.current_kind() != Some(TokenKind::Comma) {
                                break;
                            }
                            self.cursor += 1;
                        }
                        let end = self
                            .expect(
                                TokenKind::RightBrace,
                                "MNP142",
                                "expected '}' after payload fields",
                            )
                            .and_then(|index| self.tokens.get(index))
                            .map_or(span, |token| token.span);
                        return Some(AstExpr::FiniteVariant {
                            type_name: name,
                            variant,
                            fields,
                            span: SourceSpan::covering(&self.envelope.text, span, end),
                        });
                    }
                    let base = AstExpr::FiniteVariant {
                        type_name: name,
                        variant,
                        fields: Vec::new(),
                        span,
                    };
                    self.project_chain(base)
                } else if self.current_kind() == Some(TokenKind::LeftParen) {
                    self.cursor += 1;
                    let mut arguments = Vec::new();
                    while self.current_kind() != Some(TokenKind::RightParen)
                        && self.cursor < self.significant.len()
                    {
                        arguments.push(self.expression()?);
                        if self.current_kind() != Some(TokenKind::Comma) {
                            break;
                        }
                        self.cursor += 1;
                    }
                    let end = self
                        .expect(
                            TokenKind::RightParen,
                            "MNP066",
                            "expected ')' after call arguments",
                        )
                        .and_then(|index| self.tokens.get(index))
                        .map_or(name.span, |token| token.span);
                    let call = AstExpr::Call {
                        function: name.clone(),
                        arguments,
                        span: SourceSpan::covering(&self.envelope.text, name.span, end),
                    };
                    self.project_chain(call)
                } else {
                    self.project_chain(AstExpr::Name(name))
                };
                Some(primary_expr)
            }
            Some(TokenKind::IntegerLiteral) => {
                let text = self.spanned(
                    TokenKind::IntegerLiteral,
                    "MNP063",
                    "expected integer literal",
                )?;
                let value = text.text.parse().unwrap_or(0);
                Some(AstExpr::Integer { text, value })
            }
            Some(TokenKind::TrueKeyword | TokenKind::FalseKeyword) => {
                let kind = self.current_kind().expect("matched keyword");
                let text = self.spanned(kind, "MNP067", "expected boolean literal")?;
                Some(AstExpr::Boolean {
                    value: kind == TokenKind::TrueKeyword,
                    text,
                })
            }
            Some(TokenKind::LeftParen) => {
                self.cursor += 1;
                let value = self.expression();
                self.expect(
                    TokenKind::RightParen,
                    "MNP068",
                    "expected ')' after nested expression",
                );
                value
            }
            Some(TokenKind::MatchKeyword) => self.match_expression(),
            _ => {
                self.error(
                    "MNP064",
                    "expected expression",
                    vec![
                        TokenKind::Identifier,
                        TokenKind::IntegerLiteral,
                        TokenKind::TrueKeyword,
                        TokenKind::FalseKeyword,
                        TokenKind::LeftParen,
                        TokenKind::MatchKeyword,
                    ],
                );
                None
            }
        }
    }

    fn project_chain(&mut self, mut base: AstExpr) -> AstExpr {
        while self.current_kind() == Some(TokenKind::Dot) {
            self.cursor += 1;
            let Some(field) = self.spanned(
                TokenKind::Identifier,
                "MNP129",
                "expected field name after '.'",
            ) else {
                break;
            };
            let span = SourceSpan::covering(&self.envelope.text, base.span(), field.span);
            base = AstExpr::FieldProject {
                base: Box::new(base),
                field,
                span,
            };
        }
        base
    }

    /// Profile 0.5 shares `Name {` between record literals and every construct
    /// that follows an expression with a `{` block (`match` scrutinees, `if`
    /// conditions, bounded-iteration bodies). A record literal always opens
    /// with a functional update (`..base`) or a field declaration (`name :`),
    /// so bounded lookahead disambiguates without guessing.
    fn at_record_literal(&self) -> bool {
        match self.peek_kind(1) {
            Some(TokenKind::DotDot) => true,
            Some(TokenKind::Identifier) => {
                matches!(self.peek_kind(2), Some(TokenKind::Colon))
            }
            _ => false,
        }
    }

    /// After `Type.Variant`, does a `{` open a payload construction? The
    /// first token inside must start a field list (`field:`) or close the
    /// (possibly empty) payload, mirroring [`Self::at_record_literal`].
    fn at_payload_construct(&self) -> bool {
        if self.current_kind() != Some(TokenKind::LeftBrace) {
            return false;
        }
        match self.peek_kind(1) {
            Some(TokenKind::Identifier) => matches!(self.peek_kind(2), Some(TokenKind::Colon)),
            Some(TokenKind::RightBrace) => true,
            _ => false,
        }
    }

    fn peek_kind(&self, offset: usize) -> Option<TokenKind> {
        let index = *self.significant.get(self.cursor + offset)?;
        Some(self.tokens.get(index)?.kind)
    }

    fn record_literal(&mut self, type_name: SpannedText) -> Option<AstExpr> {
        self.expect(
            TokenKind::LeftBrace,
            "MNP130",
            "expected '{' after record constructor name",
        );
        let mut base: Option<Box<AstExpr>> = None;
        if self.current_kind() == Some(TokenKind::DotDot) {
            self.cursor += 1;
            let base_expr = self.expression()?;
            base = Some(Box::new(base_expr));
            if self.current_kind() != Some(TokenKind::Comma) {
                let _ = self.expect(
                    TokenKind::RightBrace,
                    "MNP131",
                    "expected '}' after record literal",
                );
                let span =
                    SourceSpan::covering(&self.envelope.text, type_name.span, type_name.span);
                return Some(AstExpr::RecordLiteral {
                    type_name,
                    base,
                    fields: Vec::new(),
                    span,
                });
            }
            self.cursor += 1;
        }
        let mut fields: Vec<(SpannedText, AstExpr)> = Vec::new();
        while self.current_kind() == Some(TokenKind::Identifier) {
            let field_name = self.spanned(
                TokenKind::Identifier,
                "MNP132",
                "expected record field name",
            )?;
            self.expect(
                TokenKind::Colon,
                "MNP133",
                "expected ':' after record field name",
            );
            let value = self.expression()?;
            fields.push((field_name, value));
            if self.current_kind() != Some(TokenKind::Comma) {
                break;
            }
            self.cursor += 1;
        }
        self.expect(
            TokenKind::RightBrace,
            "MNP131",
            "expected '}' after record literal",
        );
        let last_span = fields.last().map_or(type_name.span, |(name, value)| {
            SourceSpan::covering(&self.envelope.text, name.span, value.span())
        });
        let span = SourceSpan::covering(&self.envelope.text, type_name.span, last_span);
        Some(AstExpr::RecordLiteral {
            type_name,
            base,
            fields,
            span,
        })
    }

    fn match_expression(&mut self) -> Option<AstExpr> {
        let start = self.current_token_index();
        self.expect(TokenKind::MatchKeyword, "MNP080", "expected 'match'");
        let value = self.expression()?;
        self.expect(
            TokenKind::LeftBrace,
            "MNP081",
            "expected '{' after match value",
        );
        let mut arms = Vec::new();
        while self.current_kind() == Some(TokenKind::Identifier) {
            let first = self.spanned(TokenKind::Identifier, "MNP082", "expected match variant")?;
            // Qualified pattern `Type.VARIANT` (Profile 0.6) or the bare
            // variant name accepted by every profile.
            let mut type_name = None;
            let mut variant = first;
            if self.current_kind() == Some(TokenKind::Dot)
                && matches!(self.profile.as_str(), SOURCE_PROFILE_VERSION_0_6)
            {
                self.cursor += 1;
                type_name = Some(variant.clone());
                variant = self.spanned(
                    TokenKind::Identifier,
                    "MNP136",
                    "expected variant name after '.'",
                )?;
            }
            // Payload bindings: `VARIANT { field: binding }` with the
            // `{ field }` shorthand, or the wildcard `{ .. }` which ignores
            // the whole payload. In arm-pattern position a '{' can only open
            // bindings.
            let mut bindings = Vec::new();
            let mut ignore_payload = false;
            if self.current_kind() == Some(TokenKind::LeftBrace)
                && matches!(self.profile.as_str(), SOURCE_PROFILE_VERSION_0_6)
            {
                self.cursor += 1;
                if self.current_kind() == Some(TokenKind::DotDot) {
                    self.cursor += 1;
                    ignore_payload = true;
                }
                while self.current_kind() == Some(TokenKind::Identifier) {
                    let Some(field_name) = self.spanned(
                        TokenKind::Identifier,
                        "MNP137",
                        "expected payload field binding",
                    ) else {
                        break;
                    };
                    let binding = if self.current_kind() == Some(TokenKind::Colon) {
                        self.cursor += 1;
                        self.spanned(
                            TokenKind::Identifier,
                            "MNP138",
                            "expected binding name after ':'",
                        )?
                    } else {
                        field_name.clone()
                    };
                    bindings.push((field_name, binding));
                    if self.current_kind() != Some(TokenKind::Comma) {
                        break;
                    }
                    self.cursor += 1;
                }
                self.expect(
                    TokenKind::RightBrace,
                    "MNP139",
                    "expected '}' after payload bindings",
                );
            }
            self.expect(
                TokenKind::FatArrow,
                "MNP083",
                "expected '=>' after match variant",
            );
            let arm_value = self.expression()?;
            let span = SourceSpan::covering(&self.envelope.text, variant.span, arm_value.span());
            arms.push(AstMatchArm {
                type_name,
                variant,
                bindings,
                ignore_payload,
                value: arm_value,
                span,
            });
            if self.current_kind() != Some(TokenKind::Comma) {
                break;
            }
            self.cursor += 1;
        }
        self.expect(
            TokenKind::RightBrace,
            "MNP084",
            "expected '}' after match arms",
        );
        let end = self.previous_token_index(start);
        let span = SourceSpan::covering(
            &self.envelope.text,
            self.tokens
                .get(start)
                .map_or(value.span(), |token| token.span),
            self.tokens
                .get(end)
                .map_or(value.span(), |token| token.span),
        );
        Some(AstExpr::Match {
            value: Box::new(value),
            arms,
            span,
        })
    }

    fn parameter_list(&mut self, role: &str) -> (CstNode, Vec<AstParameter>) {
        let start = self.current_token_index();
        self.expect(
            TokenKind::LeftParen,
            "MNP020",
            &format!("expected '(' before {role} parameters"),
        );
        let mut parameters = Vec::new();
        let mut children = Vec::new();
        while self.current_kind() == Some(TokenKind::Identifier) {
            let parameter_start = self.current_token_index();
            let name = self.spanned(TokenKind::Identifier, "MNP021", "expected parameter name");
            self.expect(
                TokenKind::Colon,
                "MNP022",
                "expected ':' after parameter name",
            );
            let value_type =
                self.spanned(TokenKind::Identifier, "MNP023", "expected parameter type");
            let parameter_end = self.previous_token_index(parameter_start);
            children.push(self.node(
                CstKind::Parameter,
                parameter_start,
                parameter_end,
                Vec::new(),
            ));
            if let (Some(name), Some(value_type)) = (name, value_type) {
                parameters.push(AstParameter {
                    span: SourceSpan::covering(&self.envelope.text, name.span, value_type.span),
                    name,
                    value_type,
                });
            }
            if self.current_kind() != Some(TokenKind::Comma) {
                break;
            }
            self.cursor += 1;
        }
        self.expect(
            TokenKind::RightParen,
            "MNP024",
            &format!("expected ')' after {role} parameters"),
        );
        let end = self.previous_token_index(start);
        (
            self.node(CstKind::ParameterList, start, end, children),
            parameters,
        )
    }

    fn qualified_name(&mut self) -> Option<SpannedText> {
        let first = self.name_segment("MNP030", "expected module name")?;
        let mut text = first.text.clone();
        let mut end = first.span;
        while self.current_kind() == Some(TokenKind::Dot) {
            self.cursor += 1;
            let Some(segment) =
                self.name_segment("MNP031", "expected module name segment after '.'")
            else {
                break;
            };
            text.push('.');
            text.push_str(&segment.text);
            end = segment.span;
        }
        Some(SpannedText {
            text,
            span: SourceSpan::covering(&self.envelope.text, first.span, end),
        })
    }

    /// One dotted-name segment. The header keyword `mncs` is accepted here so
    /// standard-library modules can be named `mncs.core.*`; no earlier profile
    /// admitted such names, so this is purely additive.
    fn name_segment(&mut self, code: &str, message: &str) -> Option<SpannedText> {
        match self.current_kind() {
            Some(TokenKind::Identifier) | Some(TokenKind::MncsKeyword) => {}
            _ => {
                self.error(code, message, vec![TokenKind::Identifier]);
                return None;
            }
        }
        let index = self.significant[self.cursor];
        let token = &self.tokens[index];
        let segment = SpannedText {
            text: token.text.clone(),
            span: token.span,
        };
        self.cursor += 1;
        Some(segment)
    }

    fn spanned(&mut self, kind: TokenKind, code: &str, message: &str) -> Option<SpannedText> {
        let index = self.expect(kind, code, message)?;
        let token = &self.tokens[index];
        Some(SpannedText {
            text: token.text.clone(),
            span: token.span,
        })
    }

    fn expect(&mut self, kind: TokenKind, code: &str, message: &str) -> Option<usize> {
        if self.current_kind() == Some(kind) {
            let index = self.significant[self.cursor];
            self.cursor += 1;
            return Some(index);
        }
        self.error(code, message, vec![kind]);
        None
    }

    fn error(&mut self, code: &str, message: &str, expected: Vec<TokenKind>) {
        let (span, found) = self.current_token().map_or_else(
            || {
                (
                    SourceSpan::at(
                        &self.envelope.text,
                        self.envelope.text.len(),
                        self.envelope.text.len(),
                    ),
                    None,
                )
            },
            |token| (token.span, Some(token.kind)),
        );
        self.diagnostics.push(SourceDiagnostic {
            code: code.to_owned(),
            stage: DiagnosticStage::Parsing,
            severity: DiagnosticSeverity::Error,
            message: message.to_owned(),
            span,
            expected,
            found,
        });
    }

    fn current_token(&self) -> Option<&SourceToken> {
        self.significant
            .get(self.cursor)
            .map(|index| &self.tokens[*index])
    }

    fn current_kind(&self) -> Option<TokenKind> {
        self.current_token().map(|token| {
            let iteration_keyword = !profile_at_least(&self.profile, SOURCE_PROFILE_VERSION_0_4)
                && matches!(
                    token.kind,
                    TokenKind::IterateKeyword
                        | TokenKind::UpToKeyword
                        | TokenKind::CarryingKeyword
                        | TokenKind::NextKeyword
                        | TokenKind::WhileKeyword
                );
            // `record` is reserved from Profile 0.5 onward; earlier profiles
            // keep treating it as an ordinary identifier.
            let record_keyword = !profile_at_least(&self.profile, SOURCE_PROFILE_VERSION_0_5)
                && token.kind == TokenKind::RecordKeyword;
            // `use` is reserved from Profile 0.6 onward.
            let use_keyword = !profile_at_least(&self.profile, SOURCE_PROFILE_VERSION_0_6)
                && token.kind == TokenKind::UseKeyword;
            if iteration_keyword || record_keyword || use_keyword {
                TokenKind::Identifier
            } else {
                token.kind
            }
        })
    }

    fn current_token_index(&self) -> usize {
        self.significant
            .get(self.cursor)
            .copied()
            .unwrap_or(self.tokens.len())
    }

    fn previous_token_index(&self, fallback: usize) -> usize {
        self.cursor
            .checked_sub(1)
            .and_then(|index| self.significant.get(index))
            .map_or(fallback, |index| index + 1)
    }

    fn node(
        &self,
        kind: CstKind,
        token_start: usize,
        token_end: usize,
        children: Vec<CstNode>,
    ) -> CstNode {
        let start = self
            .tokens
            .get(token_start)
            .map_or(self.envelope.text.len(), |token| token.span.start);
        let end = token_end
            .checked_sub(1)
            .and_then(|index| self.tokens.get(index))
            .map_or(start, |token| token.span.end);
        CstNode {
            kind,
            span: SourceSpan::at(&self.envelope.text, start, end),
            token_start,
            token_end,
            children,
        }
    }
}

fn is_identifier_start(value: char) -> bool {
    value == '_' || value.is_alphabetic()
}

fn binary_operator(kind: Option<TokenKind>) -> Option<(AstBinaryOp, u8)> {
    Some(match kind? {
        TokenKind::OrOr => (AstBinaryOp::Or, 1),
        TokenKind::AndAnd => (AstBinaryOp::And, 2),
        TokenKind::EqEq => (AstBinaryOp::Eq, 3),
        TokenKind::NotEq => (AstBinaryOp::Ne, 3),
        TokenKind::Lt => (AstBinaryOp::Lt, 4),
        TokenKind::Le => (AstBinaryOp::Le, 4),
        TokenKind::Gt => (AstBinaryOp::Gt, 4),
        TokenKind::Ge => (AstBinaryOp::Ge, 4),
        TokenKind::Plus => (AstBinaryOp::Add, 5),
        TokenKind::Minus => (AstBinaryOp::Sub, 5),
        TokenKind::Star => (AstBinaryOp::Mul, 6),
        TokenKind::Slash => (AstBinaryOp::Div, 6),
        TokenKind::Percent => (AstBinaryOp::Mod, 6),
        _ => return None,
    })
}

fn is_identifier_continue(value: char) -> bool {
    value == '_' || value.is_alphanumeric()
}

pub fn source_profile_supported(version: &str) -> bool {
    matches!(
        version,
        SOURCE_PROFILE_VERSION
            | SOURCE_PROFILE_VERSION_0_2
            | SOURCE_PROFILE_VERSION_0_3
            | SOURCE_PROFILE_VERSION_0_4
            | SOURCE_PROFILE_VERSION_0_5
            | SOURCE_PROFILE_VERSION_0_6
    )
}

fn infer_source_profile(text: &str) -> &'static str {
    let header = text.lines().find(|line| {
        let trimmed = line.trim_start();
        !trimmed.is_empty() && !trimmed.starts_with("//") && !trimmed.starts_with("/*")
    });
    match header {
        Some(line) if line.trim_start().starts_with("mncs 0.6") => SOURCE_PROFILE_VERSION_0_6,
        Some(line) if line.trim_start().starts_with("mncs 0.5") => SOURCE_PROFILE_VERSION_0_5,
        Some(line) if line.trim_start().starts_with("mncs 0.4") => SOURCE_PROFILE_VERSION_0_4,
        Some(line) if line.trim_start().starts_with("mncs 0.3") => SOURCE_PROFILE_VERSION_0_3,
        Some(line) if line.trim_start().starts_with("mncs 0.2") => SOURCE_PROFILE_VERSION_0_2,
        _ => SOURCE_PROFILE_VERSION,
    }
}

fn fingerprint<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("source artifact is serializable");
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> SourceEnvelope {
        SourceEnvelope::inline(
            SourceArtifactKind::Program,
            "identity",
            "mncs 0.1;\n// retained trivia\nmodule example.identity;\nfn identity(value: i64) -> (result: i64) { return value; }\n",
        )
    }

    #[test]
    fn token_stream_and_cst_are_lossless() {
        let envelope = fixture();
        let parsed = parse(&envelope);
        assert!(parsed.is_valid(), "{:#?}", parsed.diagnostics);
        assert_eq!(parsed.lexical.reconstruct(), envelope.text);
        assert_eq!(parsed.cst.reconstruct(), envelope.text);
        assert!(parsed
            .lexical
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::LineComment));
    }

    #[test]
    fn ast_retains_names_and_byte_spans() {
        let parsed = parse(&fixture());
        let ast = parsed.ast.expect("valid AST");
        assert_eq!(ast.module.text, "example.identity");
        assert_eq!(ast.functions[0].name.text, "identity");
        assert_eq!(ast.functions[0].inputs[0].value_type.text, "i64");
        let span = ast.functions[0].body.returned_value.span();
        assert_eq!(&fixture().text[span.start..span.end], "value");
    }

    #[test]
    fn profile_05_disambiguates_record_literals_from_block_openers() {
        let envelope = SourceEnvelope::inline(
            SourceArtifactKind::Program,
            "branching",
            "mncs 0.5;\nmodule example.branch;\nenum S { PASS, FAIL }\nrecord T { v: i32 }\nfn pick(s: S, ok: bool, base: T) -> (result: i32) { return match s { PASS => 1, FAIL => 0 }; } fn branch(ok: bool) -> (result: i32) { if ok { return 1; } return 0; } fn loop_from_record(n: i32) -> (result: i32) { iterate steps up_to 2 carrying acc: T = T { v: n } { next acc = T { ..acc, v: acc.v + 1 }; } return acc.v; } fn literal(base: T) -> (result: i32) { let updated: T = T { ..base, v: 7 }; return updated.v; }\n",
        );
        let parsed = parse(&envelope);
        assert!(parsed.is_valid(), "{:#?}", parsed.diagnostics);
    }

    #[test]
    fn declarations_may_be_interleaved_across_kinds() {
        let envelope = SourceEnvelope::inline(
            SourceArtifactKind::Program,
            "interleaved",
            "mncs 0.5;\nmodule example.interleaved;\nrecord P { l: i32, r: i32 }\nenum Pick { LEFT, RIGHT, TIE }\nfn classify(l: i32, r: i32) -> (result: Pick) { let p: P = P { l: l, r: r }; if p.l > p.r { return Pick.LEFT; } return Pick.RIGHT; }\nrecord Tally { left: i32, right: i32 }\nfn tally(pick: Pick) -> (result: Tally) { return match pick { LEFT => Tally { left: 1, right: 0 }, RIGHT => Tally { left: 0, right: 1 }, TIE => Tally { left: 0, right: 0 } }; }\nenum Mode { ON, OFF }\nfn mode(open: bool) -> (result: Mode) { if open { return Mode.ON; } return Mode.OFF; }\n",
        );
        let parsed = parse(&envelope);
        assert!(parsed.is_valid(), "{:#?}", parsed.diagnostics);
        let ast = parsed.ast.expect("valid AST");
        assert_eq!(ast.record_types.len(), 2);
        assert_eq!(ast.finite_types.len(), 2);
        assert_eq!(ast.functions.len(), 3);
    }

    #[test]
    fn profile_03_interleaves_enums_and_functions_only() {
        let envelope = SourceEnvelope::inline(
            SourceArtifactKind::Program,
            "interleaved-03",
            "mncs 0.3;\nmodule example.interleaved03;\nenum E { A, B }\nfn f(e: E) -> (result: i32) { return match e { A => 1, B => 2 }; }\nenum F { C, D }\nfn g(e: F) -> (result: i32) { return match e { C => 3, D => 4 }; }\n",
        );
        let parsed = parse(&envelope);
        assert!(parsed.is_valid(), "{:#?}", parsed.diagnostics);
    }

    #[test]
    fn reports_structured_recovery_diagnostic() {
        let envelope = SourceEnvelope::inline(
            SourceArtifactKind::Program,
            "broken",
            "mncs 0.1; module broken; fn id(value: i64) -> (result: i64) { return value }",
        );
        let parsed = parse(&envelope);
        assert!(!parsed.is_valid());
        let diagnostic = parsed
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "MNP016")
            .expect("missing-semicolon diagnostic");
        assert_eq!(diagnostic.expected, vec![TokenKind::Semicolon]);
        assert_eq!(diagnostic.found, Some(TokenKind::RightBrace));
    }

    #[test]
    fn envelope_identity_binds_non_file_relationships_and_provenance() {
        let mut envelope = fixture()
            .with_relationships(vec![
                SourceRelationship {
                    kind: SourceRelationshipKind::DependsOn,
                    target_identity: "mncs:model:fraud-detector:7".to_owned(),
                },
                SourceRelationship {
                    kind: SourceRelationshipKind::Verifies,
                    target_identity: "mncs:verification-system:model-safety:3".to_owned(),
                },
            ])
            .with_provenance(SourceProvenance {
                producer_identity: Some("mncs:machine-process:source-generator:2".to_owned()),
                derived_from: vec!["mncs:workflow:model-release:11".to_owned()],
            });

        assert!(envelope.identity_is_valid());
        assert_eq!(envelope.relationships.len(), 2);
        "mncs:model:substituted:8".clone_into(&mut envelope.relationships[0].target_identity);
        assert!(!envelope.identity_is_valid());
    }
}
