pub mod contextual_effect;
pub mod no_alert;
pub mod no_console_log;
pub mod no_set_state_in_effect;
pub mod prefer_functional_transforms;
#[cfg(test)]
mod tests;

use crate::{rule::Rule, rule_engine::RuleEngine};

pub const IDS: &[&str] = &[
    "no-set-state-in-effect",
    "no-console-log",
    "no-alert",
    "contextual-effect",
    "prefer-functional-transforms",
];

pub fn select(ids: &[String]) -> Result<RuleEngine, String> {
    let mut seen = std::collections::HashSet::new();
    let mut rules: Vec<Box<dyn Rule>> = Vec::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(format!("duplicate rule id: {id}"));
        }
        rules.push(match id.as_str() {
            "no-set-state-in-effect" => Box::new(no_set_state_in_effect::NoSetStateInEffect),
            "no-console-log" => Box::new(no_console_log::NoConsoleLog),
            "no-alert" => Box::new(no_alert::NoAlert),
            "contextual-effect" => Box::new(contextual_effect::ContextualEffect),
            "prefer-functional-transforms" => {
                Box::new(prefer_functional_transforms::PreferFunctionalTransforms)
            }
            _ => {
                return Err(format!(
                    "unknown rule id: {id}; use `ai-lint rules` to list compiled rules"
                ));
            }
        });
    }
    Ok(RuleEngine::new(rules))
}
pub fn default_engine() -> RuleEngine {
    RuleEngine::new(vec![Box::new(no_set_state_in_effect::NoSetStateInEffect)])
}
