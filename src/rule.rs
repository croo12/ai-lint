//! Shared contract and owned diagnostics for AST rules.

use crate::model::ModelRequest;
use oxc_semantic::Semantic;
use oxc_span::Span;

/// Rules are compiled Rust implementations, selected by stable IDs.
pub trait Rule {
    fn id(&self) -> &'static str;
    fn check(&self, semantic: &Semantic<'_>, context: &mut RuleContext<'_>);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleViolation {
    pub rule_id: String,
    pub message: String,
    /// UTF-8 byte offsets into the original source; the end is exclusive.
    pub span: Span,
}

pub struct RuleContext<'a> {
    rule_id: &'a str,
    violations: &'a mut Vec<RuleViolation>,
    model_requests: &'a mut Vec<ModelRequest>,
}

impl<'a> RuleContext<'a> {
    pub(crate) fn new(
        rule_id: &'a str,
        violations: &'a mut Vec<RuleViolation>,
        model_requests: &'a mut Vec<ModelRequest>,
    ) -> Self {
        Self {
            rule_id,
            violations,
            model_requests,
        }
    }

    pub fn report(&mut self, span: Span, message: impl Into<String>) {
        self.violations.push(RuleViolation {
            rule_id: self.rule_id.to_owned(),
            message: message.into(),
            span,
        });
    }

    /// Queue an owned code excerpt for judgment after AST traversal completes.
    pub fn request_model(
        &mut self,
        span: Span,
        criteria: impl Into<String>,
        source: impl Into<String>,
    ) {
        self.model_requests.push(ModelRequest {
            rule_id: self.rule_id.to_owned(),
            span,
            criteria: criteria.into(),
            source: source.into(),
            message: None,
        });
    }

    pub fn request_model_with_message(
        &mut self,
        span: Span,
        criteria: impl Into<String>,
        source: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.request_model(span, criteria, source);
        self.model_requests
            .last_mut()
            .expect("request just added")
            .message = Some(message.into());
    }
}
