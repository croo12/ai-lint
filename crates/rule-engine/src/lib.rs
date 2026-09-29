//! Executes compiled Rust rules over one shared semantic AST per source file.

mod types;

pub use types::{RuleCheck, RuleInput};

use ai_lint_rule_contract::{Rule, RuleContext};
use oxc_semantic::SemanticBuilder;

#[derive(Default)]
pub struct RuleEngine {
    rules: Vec<Box<dyn Rule>>,
}
impl RuleEngine {
    pub fn new(rules: Vec<Box<dyn Rule>>) -> Self {
        Self { rules }
    }
    /// Evaluate the parsed snapshot without reparsing it. Rules still inspect
    /// the whole file; each rule may use change metadata to refine its policy.
    pub fn check(&self, input: RuleInput<'_>) -> Result<RuleCheck, String> {
        let mut check = RuleCheck::default();
        let Some(program) = input.ast else {
            return Ok(check);
        };
        if self.rules.is_empty() {
            return Ok(check);
        }
        let built = SemanticBuilder::new().with_build_nodes(true).build(program);
        if !built.diagnostics.is_empty() {
            return Err(format!("semantic analysis failed: {:?}", built.diagnostics));
        }
        for rule in &self.rules {
            if !rule.scope().includes(Some(input.path)) {
                continue;
            }
            let mut context = RuleContext::new(
                rule.id(),
                Some(input.path),
                &input.changes,
                &mut check.violations,
                &mut check.reviews,
            );
            rule.check(&built.semantic, &mut context);
        }
        Ok(check)
    }
}

#[cfg(test)]
mod tests;
