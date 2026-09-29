use crate::{
    model::ModelRequest,
    rule::{ChangeMetadata, RuleViolation},
};
use oxc_ast::ast::Program;
use std::path::Path;

/// Data required by the engine. The caller supplies a program only when parsing
/// succeeded; None skips rules for a recovered or aborted parse.
pub struct RuleInput<'a> {
    pub ast: Option<&'a Program<'a>>,
    pub path: &'a Path,
    pub changes: ChangeMetadata,
}

/// Owned findings and model requests collected during rule evaluation.
#[derive(Default)]
pub struct RuleCheck {
    pub violations: Vec<RuleViolation>,
    pub model_requests: Vec<ModelRequest>,
}
