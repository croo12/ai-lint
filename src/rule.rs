//! Shared contract and owned diagnostics for AST rules.

use crate::model::ModelRequest;
use oxc_semantic::Semantic;
use oxc_span::Span;
use std::path::Path;

/// Selects the files a rule runs on. The engine skips a rule outside its scope,
/// so the rule body never repeats the file name condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleScope {
    AllFiles,
    /// `*.test.ts` and `*.test.tsx` only.
    TestFiles,
    /// Everything except `*.test.ts` and `*.test.tsx`.
    NonTestFiles,
}

impl RuleScope {
    /// A source checked without a path cannot be proven to be a test, so only
    /// `TestFiles` opts out of it; the other scopes keep checking.
    pub fn includes(self, path: Option<&Path>) -> bool {
        match self {
            Self::AllFiles => true,
            Self::TestFiles => is_test_file(path),
            Self::NonTestFiles => !is_test_file(path),
        }
    }
}

/// Matches the file name alone so a directory named `*.test.ts` never counts.
fn is_test_file(path: Option<&Path>) -> bool {
    path.and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".test.ts") || name.ends_with(".test.tsx"))
}

/// Rules are compiled Rust implementations, selected by stable IDs.
pub trait Rule {
    fn id(&self) -> &'static str;
    /// Defaults to every checked file; override to restrict a rule to test files or to exclude them.
    fn scope(&self) -> RuleScope {
        RuleScope::AllFiles
    }
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
    file_path: Option<&'a Path>,
    violations: &'a mut Vec<RuleViolation>,
    model_requests: &'a mut Vec<ModelRequest>,
}

impl<'a> RuleContext<'a> {
    pub(crate) fn new(
        rule_id: &'a str,
        file_path: Option<&'a Path>,
        violations: &'a mut Vec<RuleViolation>,
        model_requests: &'a mut Vec<ModelRequest>,
    ) -> Self {
        Self {
            rule_id,
            file_path,
            violations,
            model_requests,
        }
    }

    /// Available when the caller checks a named source file.
    pub fn file_path(&self) -> Option<&Path> {
        self.file_path
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

#[cfg(test)]
mod tests {
    use super::RuleScope;
    use std::path::Path;

    #[test]
    fn scope_matches_test_file_names_in_both_directions() {
        for name in ["sample.test.ts", "ui/sample.browser.test.tsx"] {
            assert!(
                RuleScope::TestFiles.includes(Some(Path::new(name))),
                "{name}"
            );
            assert!(
                !RuleScope::NonTestFiles.includes(Some(Path::new(name))),
                "{name}"
            );
        }
        for name in ["sample.ts", "sample.spec.ts", "test.tsx", "a.test.ts/b.ts"] {
            assert!(
                !RuleScope::TestFiles.includes(Some(Path::new(name))),
                "{name}"
            );
            assert!(
                RuleScope::NonTestFiles.includes(Some(Path::new(name))),
                "{name}"
            );
        }
    }

    #[test]
    fn a_source_without_a_path_runs_every_scope_except_test_only() {
        assert!(RuleScope::AllFiles.includes(None));
        assert!(!RuleScope::TestFiles.includes(None));
        assert!(RuleScope::NonTestFiles.includes(None));
    }
}
