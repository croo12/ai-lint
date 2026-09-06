pub mod no_set_state_in_effect;

use crate::rule_engine::RuleEngine;
use no_set_state_in_effect::NoSetStateInEffect;

pub fn default_engine() -> RuleEngine {
    RuleEngine::new(vec![Box::new(NoSetStateInEffect)])
}
