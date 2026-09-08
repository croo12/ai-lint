//! Executes compiled Rust rules over one shared semantic AST per source file.
use crate::{
    model::{Decision, ModelClient, ModelError, ModelRequest},
    rule::{Rule, RuleContext, RuleViolation},
};
use oxc_ast::ast::Program;
use oxc_semantic::SemanticBuilder;

#[derive(Default)]
pub struct RuleCheck {
    pub violations: Vec<RuleViolation>,
    pub model_requests: Vec<ModelRequest>,
}
#[derive(Default)]
pub struct RuleEngine {
    rules: Vec<Box<dyn Rule>>,
    model: Option<Box<dyn ModelClient>>,
}
impl RuleEngine {
    pub fn new(rules: Vec<Box<dyn Rule>>) -> Self {
        Self { rules, model: None }
    }
    pub fn with_model(mut self, model: Box<dyn ModelClient>) -> Self {
        self.model = Some(model);
        self
    }
    pub fn check(&self, program: &Program<'_>) -> Result<RuleCheck, String> {
        let mut check = RuleCheck::default();
        if self.rules.is_empty() {
            return Ok(check);
        }
        let built = SemanticBuilder::new().with_build_nodes(true).build(program);
        if !built.diagnostics.is_empty() {
            return Err(format!("semantic analysis failed: {:?}", built.diagnostics));
        }
        for rule in &self.rules {
            let mut context =
                RuleContext::new(rule.id(), &mut check.violations, &mut check.model_requests);
            rule.check(&built.semantic, &mut context);
        }
        Ok(check)
    }
    /// Model requests contain owned excerpts and run after the AST is released.
    pub fn resolve(&self, mut check: RuleCheck) -> Result<Vec<RuleViolation>, ModelError> {
        for request in check.model_requests {
            let judgment = self
                .model
                .as_ref()
                .ok_or(ModelError::NotConfigured)?
                .judge(&request)?;
            match judgment.decision {
                Decision::Violation => check.violations.push(RuleViolation {
                    rule_id: request.rule_id,
                    span: request.span,
                    message: request.message.unwrap_or(judgment.reason),
                }),
                Decision::Pass => {}
                Decision::Unknown => return Err(ModelError::Unknown(request.rule_id)),
            }
        }
        Ok(check.violations)
    }
}
