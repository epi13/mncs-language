//! Compiler-owned correspondence between source constructs and executable
//! semantic operations.
//!
//! Runtime observation events carry stable operation identities.  This map is
//! the authoritative join that gives those identities source locations; a
//! consumer must not infer locations from operation order or rendered text.

use std::collections::BTreeMap;

use mncs_model::{function_id, module_id, sha256_hex, test_declaration_id, Program, SemanticId};
use mncs_syntax::{AbstractSyntaxTree, SourceEnvelope, SourceSpan};
use serde::{Deserialize, Serialize};

pub const EXECUTION_SOURCE_MAP_SCHEMA_VERSION: &str = "mncs.execution-source-map/1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSourceMap {
    pub schema_version: String,
    pub identity: String,
    pub source_identity: String,
    pub source_profile: String,
    pub module: String,
    pub module_identity: SemanticId,
    pub functions: Vec<ExecutionSourceFunction>,
    pub blocks: Vec<ExecutionSourceBlock>,
    pub operations: Vec<ExecutionSourceOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSourceFunction {
    pub identity: SemanticId,
    pub name: String,
    pub declaration_span: SourceSpan,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_declaration_identity: Option<SemanticId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_identity: Option<SemanticId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSourceBlock {
    pub identity: SemanticId,
    pub function_identity: SemanticId,
    /// Blocks are compiler-generated control-flow units.  A block may have
    /// no unique source span; its operation correspondences remain exact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_span: Option<SourceSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSourceOperation {
    pub identity: SemanticId,
    pub function_identity: SemanticId,
    pub block_identity: SemanticId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_span: Option<SourceSpan>,
    /// `false` means the compiler associated the lowered operation with a
    /// source expression span.  `true` is reserved for genuinely synthetic
    /// operations for which no source location exists.
    pub synthetic: bool,
    pub correspondence: String,
}

impl ExecutionSourceMap {
    pub fn from_parts(
        envelope: &SourceEnvelope,
        ast: &AbstractSyntaxTree,
        program: &Program,
        operation_spans: &BTreeMap<SemanticId, SourceSpan>,
    ) -> Self {
        let mut functions = Vec::new();
        let mut blocks = Vec::new();
        let mut operations = Vec::new();
        for function in program
            .functions
            .iter()
            .filter(|function| function.home_module.is_none())
        {
            let function_identity = function_id(&program.module, &function.name);
            let Some(ast_function) = ast
                .functions
                .iter()
                .find(|candidate| candidate.name.text == function.name)
            else {
                continue;
            };
            let body_identity = function
                .body
                .as_ref()
                .map(|body| body.identity(&program.module, &function.name));
            functions.push(ExecutionSourceFunction {
                identity: function_identity.clone(),
                name: function.name.clone(),
                declaration_span: ast_function.span,
                test_declaration_identity: function
                    .is_test
                    .then(|| test_declaration_id(&program.module, &function.name)),
                body_identity,
            });
            let Some(body) = &function.body else {
                continue;
            };
            for block in &body.blocks {
                let block_identity =
                    body.block_identity(&program.module, &function.name, &block.id);
                blocks.push(ExecutionSourceBlock {
                    identity: block_identity.clone(),
                    function_identity: function_identity.clone(),
                    source_span: None,
                });
                for operation in &block.operations {
                    let identity = operation.identity(&program.module, &function.name, &block.id);
                    let has_source = operation_spans.contains_key(&identity);
                    let source_span = operation_spans.get(&identity).copied();
                    operations.push(ExecutionSourceOperation {
                        identity,
                        function_identity: function_identity.clone(),
                        block_identity: block_identity.clone(),
                        synthetic: source_span.is_none(),
                        source_span,
                        correspondence: if has_source {
                            "semantic-operation-to-source-span".to_owned()
                        } else {
                            "synthetic-operation-no-source-span".to_owned()
                        },
                    });
                }
            }
        }
        functions.sort_by(|left, right| left.identity.cmp(&right.identity));
        blocks.sort_by(|left, right| left.identity.cmp(&right.identity));
        operations.sort_by(|left, right| left.identity.cmp(&right.identity));
        let mut map = Self {
            schema_version: EXECUTION_SOURCE_MAP_SCHEMA_VERSION.to_owned(),
            identity: String::new(),
            source_identity: envelope.identity.clone(),
            source_profile: ast.language_version.text.clone(),
            module: program.module.clone(),
            module_identity: module_id(&program.module),
            functions,
            blocks,
            operations,
        };
        map.seal();
        map
    }

    pub fn fingerprint(&self) -> String {
        let mut copy = self.clone();
        copy.identity.clear();
        let bytes = serde_json::to_vec(&copy).expect("source map is serializable");
        sha256_hex(&bytes)
    }

    pub fn identity_is_valid(&self) -> bool {
        self.schema_version == EXECUTION_SOURCE_MAP_SCHEMA_VERSION
            && !self.identity.is_empty()
            && self.identity == self.expected_identity()
    }

    fn expected_identity(&self) -> String {
        let mut copy = self.clone();
        copy.identity.clear();
        let bytes = serde_json::to_vec(&copy).expect("source map is serializable");
        format!("mncs:compiler:execution-source-map:{}", sha256_hex(&bytes))
    }

    fn seal(&mut self) {
        self.identity = self.expected_identity();
    }

    pub fn operation(&self, identity: &SemanticId) -> Option<&ExecutionSourceOperation> {
        self.operations
            .iter()
            .find(|operation| &operation.identity == identity)
    }
}
