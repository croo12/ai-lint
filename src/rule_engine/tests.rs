use super::{RuleEngine, RuleInput};
use crate::rule::{ChangeKind, ChangeMetadata, ChangedRange, Rule, RuleContext};
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_semantic::Semantic;
use oxc_span::{SourceType, Span};
use std::path::Path;

struct InputRule;

impl Rule for InputRule {
    fn id(&self) -> &'static str {
        "input-rule"
    }

    fn check(&self, semantic: &Semantic<'_>, context: &mut RuleContext<'_>) {
        assert_eq!(semantic.source_text(), "alert(2);");
        assert_eq!(context.file_path(), Some(Path::new("independent.ts")));
        assert_eq!(context.changes().kind, ChangeKind::Modified);
        let range = context.changes().range.as_ref().unwrap();
        assert_eq!(range.before, 6..7);
        assert_eq!(range.after, 6..7);
        context.report(Span::new(6, 7), "changed argument");
    }
}

#[test]
fn accepts_caller_supplied_input_without_an_analyzer() {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, "alert(2);", SourceType::ts()).parse();
    assert!(parsed.diagnostics.is_empty());
    let engine = RuleEngine::new(vec![Box::new(InputRule)]);
    let check = engine
        .check(RuleInput {
            ast: Some(&parsed.program),
            path: Path::new("independent.ts"),
            changes: ChangeMetadata {
                kind: ChangeKind::Modified,
                previous_bytes: Some(9),
                current_bytes: 9,
                range: Some(ChangedRange {
                    before: 6..7,
                    after: 6..7,
                }),
            },
        })
        .unwrap();
    let violations = engine.resolve(check).unwrap();
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].rule_id, "input-rule");
    assert_eq!(violations[0].span, Span::new(6, 7));
}
