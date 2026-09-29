//! Connects analysis output to engine input. Neither component knows the other.

use crate::{
    analyzer::{AnalyzedFile, ChangeKind as AnalyzedChangeKind},
    rule::{ChangeKind, ChangeMetadata, ChangedRange},
    rule_engine::RuleInput,
};

impl<'a> From<&'a AnalyzedFile<'a>> for RuleInput<'a> {
    fn from(analyzed: &'a AnalyzedFile<'a>) -> Self {
        Self {
            ast: analyzed.is_valid().then_some(&analyzed.ast),
            path: analyzed.path,
            changes: ChangeMetadata {
                kind: match analyzed.changes.kind {
                    AnalyzedChangeKind::Unknown => ChangeKind::Unknown,
                    AnalyzedChangeKind::Added => ChangeKind::Added,
                    AnalyzedChangeKind::Modified => ChangeKind::Modified,
                    AnalyzedChangeKind::Unchanged => ChangeKind::Unchanged,
                },
                previous_bytes: analyzed.changes.previous_bytes,
                current_bytes: analyzed.changes.current_bytes,
                range: analyzed.changes.range.as_ref().map(|range| ChangedRange {
                    before: range.before.clone(),
                    after: range.after.clone(),
                }),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        analyzer::{Allocator, Analyzer, ChangedFile},
        rule::{Rule, RuleContext},
        rule_engine::RuleEngine,
        rules,
    };
    use oxc_semantic::Semantic;
    use oxc_span::Span;
    use std::path::Path;

    struct ChangeAwareRule;

    #[test]
    fn maps_all_change_states_and_preserves_the_ast_without_copying_it() {
        for (file, expected_kind, expected_size, expected_range) in [
            (
                ChangedFile::new("test.ts", "alert(2);"),
                ChangeKind::Unknown,
                None,
                None,
            ),
            (
                ChangedFile::added("test.ts", "alert(2);"),
                ChangeKind::Added,
                None,
                Some(ChangedRange {
                    before: 0..0,
                    after: 0..9,
                }),
            ),
            (
                ChangedFile::new("test.ts", "alert(2);").with_previous_source("alert(1);"),
                ChangeKind::Modified,
                Some(9),
                Some(ChangedRange {
                    before: 6..7,
                    after: 6..7,
                }),
            ),
            (
                ChangedFile::new("test.ts", "alert(2);").with_previous_source("alert(2);"),
                ChangeKind::Unchanged,
                Some(9),
                None,
            ),
        ] {
            let allocator = Allocator::default();
            let analyzed = Analyzer::analyze(&allocator, &file).unwrap();
            let input = RuleInput::from(&analyzed);
            assert!(std::ptr::eq(input.ast.unwrap(), &analyzed.ast));
            assert_eq!(input.path, analyzed.path);
            assert_eq!(input.changes.kind, expected_kind);
            assert_eq!(input.changes.previous_bytes, expected_size);
            assert_eq!(input.changes.current_bytes, 9);
            assert_eq!(input.changes.range, expected_range);
        }
    }

    impl Rule for ChangeAwareRule {
        fn id(&self) -> &'static str {
            "change-aware"
        }

        fn check(&self, semantic: &Semantic<'_>, context: &mut RuleContext<'_>) {
            assert_eq!(context.file_path(), Some(Path::new("test.ts")));
            assert_eq!(semantic.source_text(), "alert(2);");
            assert_eq!(context.changes().kind, ChangeKind::Modified);
            let range = context.changes().range.as_ref().unwrap();
            assert_eq!(range.before, 6..7);
            assert_eq!(range.after, 6..7);
            context.report(
                Span::new(range.after.start as u32, range.after.end as u32),
                "changed",
            );
        }
    }

    #[test]
    fn passes_the_ast_path_and_change_metadata_to_rules() {
        let file = ChangedFile::new("test.ts", "alert(2);").with_previous_source("alert(1);");
        let allocator = Allocator::default();
        let analyzed = Analyzer::analyze(&allocator, &file).unwrap();
        let engine = RuleEngine::new(vec![Box::new(ChangeAwareRule)]);
        let violations = engine
            .resolve(engine.check((&analyzed).into()).unwrap())
            .unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].span, Span::new(6, 7));
    }

    #[test]
    fn one_analysis_can_be_consumed_by_different_rule_engines() {
        let file = ChangedFile::new("test.ts", "alert(1); console.log(2);");
        let allocator = Allocator::default();
        let analyzed = Analyzer::analyze(&allocator, &file).unwrap();
        for id in ["no-alert", "no-console-log"] {
            let engine = rules::select(&[id.into()]).unwrap();
            let violations = engine
                .resolve(engine.check((&analyzed).into()).unwrap())
                .unwrap();
            assert_eq!(violations.len(), 1);
            assert_eq!(violations[0].rule_id, id);
        }
    }

    struct MustNotRun;

    impl Rule for MustNotRun {
        fn id(&self) -> &'static str {
            "must-not-run"
        }

        fn check(&self, _: &Semantic<'_>, _: &mut RuleContext<'_>) {
            panic!("rules must not inspect an invalid AST");
        }
    }

    #[test]
    fn invalid_or_panicked_analysis_does_not_run_rules_or_request_a_model() {
        let engine = RuleEngine::new(vec![Box::new(MustNotRun)]);
        for source in ["const x: = 1;", "const x = 1;"] {
            let file = ChangedFile::new("test.ts", source);
            let allocator = Allocator::default();
            let mut analyzed = Analyzer::analyze(&allocator, &file).unwrap();
            if analyzed.syntax_errors.is_empty() {
                analyzed.panicked = true;
            }
            let check = engine.check((&analyzed).into()).unwrap();
            assert!(check.violations.is_empty());
            assert!(check.model_requests.is_empty());
            assert!(engine.resolve(check).unwrap().is_empty());
        }
    }

    #[test]
    fn change_metadata_does_not_implicitly_filter_existing_rules() {
        let file = ChangedFile::new("test.ts", "alert(1); const value = 2;")
            .with_previous_source("alert(1); const value = 1;");
        let allocator = Allocator::default();
        let analyzed = Analyzer::analyze(&allocator, &file).unwrap();
        let engine = rules::select(&["no-alert".into()]).unwrap();
        let violations = engine
            .resolve(engine.check((&analyzed).into()).unwrap())
            .unwrap();
        assert_eq!(violations.len(), 1);
        assert!(violations[0].span.end as usize <= analyzed.changes.range.unwrap().after.start);
    }
}
