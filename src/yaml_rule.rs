//! YAML is the sole rule authoring interface. Rust implements generic queries.
mod document;
mod query;
#[cfg(test)]
mod tests;

use crate::{rule::RuleContext, rule_engine::RuleEngine};
pub(crate) use document::{Document, binding_symbols};
use query::{Captures, Query};
use serde::Deserialize;
use std::{fs, path::Path};

#[derive(Debug, thiserror::Error)]
pub enum YamlRuleError {
    #[error("could not read rule: {0}")]
    Read(#[from] std::io::Error),
    #[error("invalid YAML rule: {0}")]
    Parse(#[from] serde_yaml_ng::Error),
    #[error("invalid rule: {0}")]
    Invalid(String),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YamlRule {
    version: u32,
    id: String,
    message: String,
    #[serde(rename = "match")]
    selector: Query,
    #[serde(rename = "where")]
    condition: Option<Query>,
    judge: Option<Judge>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Judge {
    #[serde(default)]
    context: ModelContext,
    criteria: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ModelContext {
    #[default]
    MatchedNode,
    EnclosingFunction,
    SourceFile,
}

pub fn default_engine() -> RuleEngine {
    RuleEngine::new(vec![
        YamlRule::parse(include_str!("../rules/no-set-state-in-effect.yaml"))
            .expect("bundled YAML must be valid"),
    ])
}

impl YamlRule {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn load(path: impl AsRef<Path>) -> Result<Self, YamlRuleError> {
        Self::parse(&fs::read_to_string(path)?)
    }
    pub fn parse(source: &str) -> Result<Self, YamlRuleError> {
        if source.len() > 256 * 1024 {
            return invalid("rule file exceeds 256 KiB");
        }
        let rule: Self = serde_yaml_ng::from_str(source)?;
        if rule.version != 2 {
            return invalid(
                "only version 2 is supported; migrate legacy selectors to generic queries",
            );
        }
        if rule.id.is_empty()
            || !rule
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_/".contains(&c))
        {
            return invalid("id must contain only ASCII letters, digits, -, _, or /");
        }
        if rule.message.trim().is_empty() {
            return invalid("message must not be empty");
        }
        let captures = rule.selector.validate(0, &Default::default())?;
        if let Some(condition) = &rule.condition {
            condition.validate(0, &captures)?;
        }
        if rule
            .judge
            .as_ref()
            .is_some_and(|j| j.criteria.trim().is_empty())
        {
            return invalid("judge.criteria must not be empty");
        }
        Ok(rule)
    }
    pub(crate) fn check(
        &self,
        document: &Document<'_>,
        context: &mut RuleContext<'_>,
    ) -> Result<(), String> {
        let mut budget = 1_000_000usize;
        for node in 0..document.nodes.len() {
            let candidates =
                self.selector
                    .evaluate(document, node, Captures::new(), &mut budget)?;
            let mut matched = false;
            for captures in candidates {
                if let Some(condition) = &self.condition
                    && condition
                        .evaluate(document, node, captures, &mut budget)?
                        .is_empty()
                {
                    continue;
                }
                matched = true;
                break;
            }
            if !matched {
                continue;
            }
            let span = document.span(node);
            if let Some(judge) = &self.judge {
                let excerpt = match judge.context {
                    ModelContext::SourceFile => document.source,
                    ModelContext::MatchedNode => document.excerpt(node),
                    ModelContext::EnclosingFunction => {
                        document.excerpt(document.enclosing_function(node))
                    }
                };
                context.request_model_with_message(span, &judge.criteria, excerpt, &self.message);
            } else {
                context.report(span, &self.message);
            }
        }
        Ok(())
    }
}
fn invalid<T>(message: &str) -> Result<T, YamlRuleError> {
    Err(YamlRuleError::Invalid(message.into()))
}
