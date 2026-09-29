//! Shared rule contracts and owned results, independent of concrete rules and evaluators.

use oxc_semantic::Semantic;
use oxc_span::Span;
use std::{ops::Range, path::Path};

/// Change information understood by rules, independent of the source analyzer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Unknown,
    Added,
    Modified,
    Unchanged,
}

/// UTF-8 byte ranges enclosing the edits; ends are exclusive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedRange {
    pub before: Range<usize>,
    pub after: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeMetadata {
    pub kind: ChangeKind,
    pub previous_bytes: Option<usize>,
    pub current_bytes: usize,
    /// None when contents are unchanged or the baseline is unavailable.
    pub range: Option<ChangedRange>,
}

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

/// A rule candidate requiring judgment after AST traversal. The caller chooses
/// how to evaluate it; this contract does not depend on an AI client.
#[derive(Debug, Clone)]
pub struct ReviewRequest {
    pub rule_id: String,
    pub span: Span,
    pub criteria: String,
    pub source: String,
    /// Optional rule-authored output instead of the evaluator's explanation.
    pub message: Option<String>,
}

pub struct RuleContext<'a> {
    rule_id: &'a str,
    file_path: Option<&'a Path>,
    changes: &'a ChangeMetadata,
    violations: &'a mut Vec<RuleViolation>,
    reviews: &'a mut Vec<ReviewRequest>,
}

impl<'a> RuleContext<'a> {
    /// Create a context for one rule invocation and collect its owned results.
    pub fn new(
        rule_id: &'a str,
        file_path: Option<&'a Path>,
        changes: &'a ChangeMetadata,
        violations: &'a mut Vec<RuleViolation>,
        reviews: &'a mut Vec<ReviewRequest>,
    ) -> Self {
        Self {
            rule_id,
            file_path,
            changes,
            violations,
            reviews,
        }
    }

    /// Available when the caller checks a named source file.
    pub fn file_path(&self) -> Option<&Path> {
        self.file_path
    }

    pub fn changes(&self) -> &ChangeMetadata {
        self.changes
    }

    pub fn report(&mut self, span: Span, message: impl Into<String>) {
        self.violations.push(RuleViolation {
            rule_id: self.rule_id.to_owned(),
            message: message.into(),
            span,
        });
    }

    /// Queue an owned code excerpt for judgment after AST traversal completes.
    pub fn request_review(
        &mut self,
        span: Span,
        criteria: impl Into<String>,
        source: impl Into<String>,
    ) {
        self.reviews.push(ReviewRequest {
            rule_id: self.rule_id.to_owned(),
            span,
            criteria: criteria.into(),
            source: source.into(),
            message: None,
        });
    }

    pub fn request_review_with_message(
        &mut self,
        span: Span,
        criteria: impl Into<String>,
        source: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.request_review(span, criteria, source);
        self.reviews.last_mut().expect("request just added").message = Some(message.into());
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
