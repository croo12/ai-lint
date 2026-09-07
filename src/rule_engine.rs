//! Executes YAML queries over one shared AST and binding index per source file.
use crate::{
    model::{Decision, ModelClient, ModelError, ModelRequest},
    rule::{RuleContext, RuleViolation},
    yaml_rule::{Document, YamlRule, binding_symbols},
};
use oxc_ast::ast::Program;

#[derive(Default)]
pub struct RuleCheck {
    pub violations: Vec<RuleViolation>,
    pub model_requests: Vec<ModelRequest>,
}
#[derive(Default)]
pub struct RuleEngine {
    rules: Vec<YamlRule>,
    model: Option<Box<dyn ModelClient>>,
}
impl RuleEngine {
    pub fn new(rules: Vec<YamlRule>) -> Self {
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
        let bindings = binding_symbols(program)?;
        let tree = serde_json::from_str(&program.to_estree_json(true, false))
            .map_err(|error| format!("could not index AST: {error}"))?;
        let document = Document::new(program.source_text, &tree, bindings);
        for rule in &self.rules {
            let mut context =
                RuleContext::new(rule.id(), &mut check.violations, &mut check.model_requests);
            rule.check(&document, &mut context)
                .map_err(|error| format!("{}: {error}", rule.id()))?;
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
