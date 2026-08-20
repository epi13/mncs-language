use std::fmt::Write;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SOURCE_ENVELOPE_SCHEMA_VERSION: &str = "0.1";
pub const SOURCE_PROFILE_VERSION: &str = "0.1";
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
        let mut envelope = Self {
            schema_version: SOURCE_ENVELOPE_SCHEMA_VERSION.to_owned(),
            identity: String::new(),
            language_version: SOURCE_PROFILE_VERSION.to_owned(),
            artifact_kind,
            logical_name: logical_name.into(),
            origin,
            relationships: Vec::new(),
            provenance: SourceProvenance::default(),
            text: text.into(),
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
            && self.language_version == SOURCE_PROFILE_VERSION
            && !self.logical_name.trim().is_empty()
            && self
                .relationships
                .iter()
                .all(|relationship| !relationship.target_identity.trim().is_empty())
            && self
                .provenance
                .producer_identity
                .as_ref()
                .map_or(true, |identity| !identity.trim().is_empty())
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
    Identifier,
    Version,
    Dot,
    Colon,
    Semicolon,
    Comma,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    Arrow,
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
    FunctionDeclaration,
    ParameterList,
    Parameter,
    Block,
    ReturnStatement,
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
    pub returned_value: SpannedText,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstFunction {
    pub name: SpannedText,
    pub inputs: Vec<AstParameter>,
    pub outputs: Vec<AstParameter>,
    pub body: AstBody,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbstractSyntaxTree {
    pub schema_version: String,
    pub source_identity: String,
    pub language_version: SpannedText,
    pub module: SpannedText,
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
                _ => TokenKind::Identifier,
            }
        } else if current.is_ascii_digit() {
            offset += current.len_utf8();
            while offset < source.len()
                && source[offset..]
                    .chars()
                    .next()
                    .is_some_and(|value| value.is_ascii_digit() || value == '.')
            {
                offset += 1;
            }
            TokenKind::Version
        } else {
            offset += current.len_utf8();
            match current {
                '.' => TokenKind::Dot,
                ':' => TokenKind::Colon,
                ';' => TokenKind::Semicolon,
                ',' => TokenKind::Comma,
                '(' => TokenKind::LeftParen,
                ')' => TokenKind::RightParen,
                '{' => TokenKind::LeftBrace,
                '}' => TokenKind::RightBrace,
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
        }
    }

    fn document(&mut self) -> (ConcreteSyntaxTree, Option<AbstractSyntaxTree>) {
        let header_start = self.current_token_index();
        let header_keyword = self.expect(TokenKind::MncsKeyword, "MNP001", "expected MNCS header");
        let version = self.spanned(TokenKind::Version, "MNP002", "expected language version");
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

        let mut functions = Vec::new();
        let mut function_nodes = Vec::new();
        while self.current_kind() == Some(TokenKind::FunctionKeyword) {
            let (node, function) = self.function();
            function_nodes.push(node);
            if let Some(function) = function {
                functions.push(function);
            }
        }
        if functions.is_empty() {
            self.error(
                "MNP006",
                "source profile requires at least one function",
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
        children.extend(function_nodes);
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
            (Some(_), Some(language_version), Some(module)) if !functions.is_empty() => {
                Some(AbstractSyntaxTree {
                    schema_version: AST_SCHEMA_VERSION.to_owned(),
                    source_identity: self.envelope.identity.clone(),
                    language_version,
                    module,
                    functions,
                    span: source_span,
                })
            }
            _ => None,
        };
        (cst, ast)
    }

    fn function(&mut self) -> (CstNode, Option<AstFunction>) {
        let start = self.current_token_index();
        self.expect(TokenKind::FunctionKeyword, "MNP010", "expected 'fn'");
        let name = self.spanned(TokenKind::Identifier, "MNP011", "expected function name");
        let (input_node, inputs) = self.parameter_list("input");
        self.expect(TokenKind::Arrow, "MNP012", "expected '->' before outputs");
        let (output_node, outputs) = self.parameter_list("output");
        let block_start = self.current_token_index();
        self.expect(TokenKind::LeftBrace, "MNP013", "expected function body");
        let return_start = self.current_token_index();
        self.expect(
            TokenKind::ReturnKeyword,
            "MNP014",
            "expected explicit return",
        );
        let returned_value = self.spanned(
            TokenKind::Identifier,
            "MNP015",
            "expected returned value name",
        );
        self.expect(TokenKind::Semicolon, "MNP016", "expected ';' after return");
        let return_end = self.previous_token_index(return_start);
        let return_node = self.node(
            CstKind::ReturnStatement,
            return_start,
            return_end,
            Vec::new(),
        );
        self.expect(
            TokenKind::RightBrace,
            "MNP017",
            "expected '}' after function body",
        );
        let end = self.previous_token_index(start);
        let block_node = self.node(CstKind::Block, block_start, end, vec![return_node]);
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
                body: AstBody {
                    returned_value,
                    span: body_span,
                },
                span: function_node.span,
            }),
            _ => None,
        };
        (function_node, function)
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
        let first = self.spanned(TokenKind::Identifier, "MNP030", "expected module name")?;
        let mut text = first.text.clone();
        let mut end = first.span;
        while self.current_kind() == Some(TokenKind::Dot) {
            self.cursor += 1;
            let Some(segment) = self.spanned(
                TokenKind::Identifier,
                "MNP031",
                "expected module name segment after '.'",
            ) else {
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
        self.current_token().map(|token| token.kind)
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

fn is_identifier_continue(value: char) -> bool {
    value == '_' || value.is_alphanumeric()
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
        let span = ast.functions[0].body.returned_value.span;
        assert_eq!(&fixture().text[span.start..span.end], "value");
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
