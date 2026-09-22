use super::*;
use crate::{
    analyzer::{AnalyzeError, Analyzer},
    model::{Decision, ModelClient, ModelError, ModelJudgment, ModelRequest},
    rule::{RuleContext, RuleScope, RuleViolation},
};
use oxc_semantic::Semantic;
use oxc_span::Span;
struct Stub;
impl ModelClient for Stub {
    fn judge(&self, _: &ModelRequest) -> Result<ModelJudgment, ModelError> {
        Ok(ModelJudgment {
            decision: Decision::Violation,
            reason: "candidate".into(),
        })
    }
}
fn check(id: &str, source: &str) -> Vec<RuleViolation> {
    let engine = select(&[id.into()]).unwrap().with_model(Box::new(Stub));
    let result = Analyzer::analyze_source_with_rules("test.tsx", source, &engine).unwrap();
    assert!(
        result.syntax_errors.is_empty(),
        "{:?}",
        result.syntax_errors
    );
    result.rule_violations
}
struct ScopedStub(RuleScope);
impl Rule for ScopedStub {
    fn id(&self) -> &'static str {
        "stub-scope"
    }
    fn scope(&self) -> RuleScope {
        self.0
    }
    fn check(&self, _: &Semantic<'_>, context: &mut RuleContext<'_>) {
        context.report(Span::new(0, 0), "stub");
    }
}

#[test]
fn the_engine_runs_a_rule_only_inside_its_scope() {
    for (scope, path, expected) in [
        (RuleScope::AllFiles, "sample.ts", 1),
        (RuleScope::AllFiles, "sample.test.ts", 1),
        (RuleScope::TestFiles, "sample.ts", 0),
        (RuleScope::TestFiles, "sample.test.ts", 1),
        (RuleScope::TestFiles, "sample.test.tsx", 1),
        (RuleScope::NonTestFiles, "sample.ts", 1),
        (RuleScope::NonTestFiles, "sample.test.ts", 0),
        (RuleScope::NonTestFiles, "sample.test.tsx", 0),
    ] {
        let engine = RuleEngine::new(vec![Box::new(ScopedStub(scope))]);
        let result = Analyzer::analyze_source_with_rules(path, "const x = 1;", &engine).unwrap();
        assert_eq!(result.rule_violations.len(), expected, "{scope:?} {path}");
    }
}

#[test]
fn create_context_is_rejected_in_every_call_form() {
    for source in [
        "import { createContext } from 'react'; const C = createContext(null);",
        "import React from 'react'; const C = React.createContext(null);",
        "import * as React from 'react'; const C = React.createContext<Value | null>(null);",
        "import { createContext as make } from 'react'; const C = make(null);",
        "const C = createContext<Value | null>(null);",
        "function useThing() { return createContext(null); }",
    ] {
        assert_eq!(check("no-create-context", source).len(), 1, "{source}");
    }
}

#[test]
fn the_safe_context_factory_and_unrelated_calls_stay_allowed() {
    for source in [
        "import { createSafeContext } from '@shared/lib'; const C = createSafeContext(null);",
        "export function createSafeContext<T>() { return createContext<T | null>(null); }",
        "export const createSafeContext = <T,>() => createContext<T | null>(null);",
        "const C = createContextMenu(null);",
        "const C = context.create(null);",
    ] {
        assert!(check("no-create-context", source).is_empty(), "{source}");
    }
}

#[test]
fn forward_ref_is_rejected_in_every_call_form() {
    for source in [
        "import { forwardRef } from 'react'; const Input = forwardRef((props, ref) => null);",
        "import React from 'react'; const Input = React.forwardRef((props, ref) => null);",
        "import * as React from 'react'; const Input = React.forwardRef<HTMLInputElement, Props>((props, ref) => null);",
        "import { forwardRef as fr } from 'react'; const Input = fr((props, ref) => null);",
        "export default forwardRef(function Input(props, ref) { return null; });",
    ] {
        assert_eq!(check("no-forward-ref", source).len(), 1, "{source}");
    }
}

#[test]
fn a_ref_prop_and_unrelated_names_stay_allowed() {
    for source in [
        "function Input({ ref, ...rest }: Props) { return null; }",
        "const Input = ({ ref }: Props) => null;",
        "const x = forwardRefs(list);",
        "const x = ref.forward(value);",
    ] {
        assert!(check("no-forward-ref", source).is_empty(), "{source}");
    }
}

#[test]
fn collection_rule_matches_loop_forms_and_function_forms_once() {
    for body in [
        "for (const x of xs) { out.push(x); out.push(x + 1); }",
        "for (let i = 0; i < xs.length; i++) out.push(xs[i]);",
        "for (const key in xs) out.push(xs[key]);",
        "while (xs.length) out.push(xs.pop());",
        "do { out.push(xs.pop()); } while (xs.length);",
        "for (const x of xs) { if (x) { out.push(x); } }",
    ] {
        for source in [
            format!("function convert(xs) {{ const out: number[] = []; {body} return out; }}"),
            format!("const convert = (xs) => {{ const out = []; {body} return out; }};"),
            format!("const convert = function(xs) {{ const out = []; {body} return out; }};"),
            format!("class Converter {{ convert(xs) {{ const out = []; {body} return out; }} }}"),
        ] {
            assert_eq!(
                check("prefer-functional-transforms", &source).len(),
                1,
                "{source}"
            );
        }
    }
}

#[test]
fn collection_rule_excludes_non_candidates_and_shadowed_bindings() {
    for source in [
        "function f(xs) { return xs.map(x => x + 1).filter(Boolean); }",
        "function f(xs, out) { for (const x of xs) out.push(x); }",
        "const out = []; function f(xs) { for (const x of xs) out.push(x); }",
        "function f(xs) { const out = []; out.push(xs); return out; }",
        "function f(xs) { for (const x of xs) { const out = []; out.push(x); } }",
        "function f(xs) { const out = []; for (const out of xs) out.push(1); }",
        "function f(xs) { const out = []; for (const x of xs) { const out = other; out.push(x); } }",
        "function f(xs) { { const out = []; } for (const x of xs) out.push(x); }",
        "function f(xs) { const out = []; for (const x of xs) { const cb = () => out.push(x); } }",
        "function f(xs) { const out = []; function inner() { for (const x of xs) out.push(x); } }",
        "function f(xs) { const out = []; for (out.push(1); false;) {} }",
        "function f(xs) { const out = []; for (const x of xs) out.push(); }",
        "function f(xs) { const out = []; for (const x of xs) other.push(x); }",
        "function f(xs) { const out = []; for (const x of xs) { const text = 'out.push(x)'; /* out.push(x) */ } }",
        "function f(xs) { const out = []; for (const x of xs) out['push'](x); }",
        "function f(xs) { const out = []; for (const x of xs) out?.push(x); }",
    ] {
        assert!(
            check("prefer-functional-transforms", source).is_empty(),
            "{source}"
        );
    }
}

#[test]
fn captures_backtrack_over_all_declarations_without_name_heuristics() {
    for source in [
        "function f(xs) { const unused = []; const out = []; for (const x of xs) out.push(x); }",
        "function f(xs) { const out = []; const unused = []; for (const x of xs) out.push(x); }",
        "function f(xs) { const out = []; { const out = other; out.push(1); } for (const x of xs) out.push(x); }",
        "function f(xs) { const out = []; function inner(out) {} for (const x of xs) out.push(x); }",
        "function f(xs) { { var out = []; } for (const x of xs) out.push(x); }",
    ] {
        assert_eq!(
            check("prefer-functional-transforms", source).len(),
            1,
            "{source}"
        );
    }
}

#[test]
fn nested_functions_and_utf8_spans_are_independent() {
    let inner =
        "function inner(xs) { const out = []; for (const x of xs) out.push(x); return out; }";
    let source = format!("const label = '한글'; function outer() {{ {inner} }}");
    let found = check("prefer-functional-transforms", &source);
    assert_eq!(found.len(), 1);
    assert_eq!(
        &source[found[0].span.start as usize..found[0].span.end as usize],
        inner
    );
}

#[test]
fn effect_policy_is_rust_and_preserves_existing_cases() {
    for source in [
        "useEffect(() => { setState(1); setState(2); }, []);",
        "React.useEffect(function () { if (ok) setState(1); }, []);",
        "const [x, update] = React.useState<number>(0); useEffect((() => update(1)), []);",
        "function App() { const [x, setX] = useState(0); useEffect(() => queueMicrotask(() => setX(1)), []); return <div />; }",
    ] {
        assert_eq!(check("no-set-state-in-effect", source).len(), 1, "{source}");
    }
    for source in [
        "setState(1); useEffect(() => {}, []);",
        "useEffect(() => { /* setState(1) */ const text = 'setState(1)'; }, []);",
        "useEffect(() => console.log(setState), [setState(1)]);",
        "other.useEffect(() => setState(1));",
        "useEffect(callback, []); useEffect();",
        "const [x, update] = useState(0); useEffect((update) => update(1));",
        "function a() { const [x, update] = useState(0); } function b() { useEffect(() => update(1)); }",
    ] {
        assert!(
            check("no-set-state-in-effect", source).is_empty(),
            "{source}"
        );
    }
}

#[test]
fn selection_rejects_unknown_and_duplicate_ids() {
    assert!(select(&["missing".into()]).is_err());
    assert!(select(&["no-alert".into(), "no-alert".into()]).is_err());
    for id in IDS {
        assert!(select(&[(*id).into()]).is_ok());
    }
}

#[test]
fn useless_comments_report_links_decisions_and_code_explanations_but_allow_described_todo() {
    let source = "// https://example.com/docs\n// TODO\n// TODO: 설명을 보완한다.\n// A 대신 B를 선택했다\n// return 결과를 반환한다\nconst result = 1;";
    let found = check("no-useless-comments", source);
    assert_eq!(found.len(), 4);
    assert!(found.iter().all(|v| v.rule_id == "no-useless-comments"));
}

#[test]
fn useless_comments_only_inspects_parser_comments_and_preserves_todo_exceptions() {
    let engine = select(&["no-useless-comments".into()]).unwrap();
    for source in [
        "const text = '// TODO';",
        "const text = `/* TODO */`;",
        "const text = '// https://example.com/docs';",
        "// TODO: A 대신 B를 선택하도록 수정한다.\nconst x = 1;",
        "/* TODO: https://example.com/docs 확인 */ const x = 1;",
        "/** TODO: 동작을 수정한다. */ const x = 1;",
        "// FIXME\n// HACK\nconst x = 1;",
    ] {
        let result = Analyzer::analyze_source_with_rules("test.ts", source, &engine).unwrap();
        assert!(result.syntax_errors.is_empty(), "{source}");
        assert!(result.rule_violations.is_empty(), "{source}");
    }
    for comment in ["// TODO:", "/* TODO */", "/**\n * TODO:\n */"] {
        let source = format!("const label = '한글';\n{comment}");
        let result = Analyzer::analyze_source_with_rules("test.ts", &source, &engine).unwrap();
        assert_eq!(result.rule_violations.len(), 1, "{comment}");
        let span = result.rule_violations[0].span;
        assert_eq!(&source[span.start as usize..span.end as usize], comment);
    }
}

#[test]
fn unsafe_type_assertions_require_a_preceding_comment_and_reject_double_assertions() {
    assert!(check("no-unsafe-type-assertions", "const value = input as const;").is_empty());
    for source in [
        "// API response is validated upstream\nconst value = input as User;",
        "/* legacy boundary */\nconst value = input as User;",
    ] {
        assert!(
            check("no-unsafe-type-assertions", source).is_empty(),
            "{source}"
        );
    }
    for source in [
        "const value = input as User;",
        "// temporary cast\nconst value = input as unknown as User;",
    ] {
        assert_eq!(
            check("no-unsafe-type-assertions", source).len(),
            1,
            "{source}"
        );
    }
}

#[test]
fn wildcard_exports_report_each_declaration_without_a_model() {
    let engine = select(&["no-wildcard-export".into()]).unwrap();
    for export in [
        "export * from 'module';",
        "export type * from 'module';",
        "export * as namespace from 'module';",
        "export type * as types from 'module';",
    ] {
        let source = format!("// 한글\n{export}");
        let result = Analyzer::analyze_source_with_rules("index.ts", &source, &engine).unwrap();
        assert!(result.syntax_errors.is_empty(), "{export}");
        assert_eq!(result.rule_violations.len(), 1, "{export}");
        let violation = &result.rule_violations[0];
        assert_eq!(violation.rule_id, "no-wildcard-export");
        assert_eq!(
            &source[violation.span.start as usize..violation.span.end as usize],
            export
        );
    }
    assert_eq!(
        check(
            "no-wildcard-export",
            "export * from 'a'; export * from 'b';"
        )
        .len(),
        2
    );
}

#[test]
fn explicit_exports_and_namespace_imports_are_allowed() {
    for source in [
        "export { foo, bar as publicBar } from 'module';",
        "export type { Foo } from 'module';",
        "const foo = 1; export { foo };",
        "export const foo = 1; export default foo;",
        "import * as namespace from 'module'; export { namespace };",
        "// export * from 'module';\nconst text = \"export * from 'module'\";",
        "export function* items() { yield 1; }",
    ] {
        assert!(check("no-wildcard-export", source).is_empty(), "{source}");
    }
}
#[test]
fn simple_rules_match_only_expected_calls() {
    assert_eq!(
        check("no-alert", "alert(1); window.alert(2); other.alert(3);").len(),
        2
    );
    assert_eq!(
        check(
            "no-console-log",
            "console.log(1); console.warn(2); other.log(3);"
        )
        .len(),
        1
    );
    assert!(check("no-alert", "const text = 'alert(1)'; // alert(2)").is_empty());
}
struct ExpectModel {
    decision: Decision,
    expected: String,
}
impl ModelClient for ExpectModel {
    fn judge(&self, request: &ModelRequest) -> Result<ModelJudgment, ModelError> {
        assert_eq!(request.source, self.expected);
        Ok(ModelJudgment {
            decision: self.decision,
            reason: "reason".into(),
        })
    }
}
#[test]
fn model_rules_send_function_context_once_and_preserve_decisions() {
    for (id, source) in [
        (
            "prefer-functional-transforms",
            "function f(xs) { const a = []; const b = []; for (const x of xs) { a.push(x); b.push(x); } }",
        ),
        (
            "contextual-effect",
            "function f() { useEffect(() => { setState(1); setState(2); }); }",
        ),
    ] {
        for decision in [Decision::Pass, Decision::Violation, Decision::Unknown] {
            let engine = select(&[id.into()])
                .unwrap()
                .with_model(Box::new(ExpectModel {
                    decision,
                    expected: source.into(),
                }));
            let result = Analyzer::analyze_source_with_rules("test.ts", source, &engine);
            match decision {
                Decision::Pass => assert!(result.unwrap().rule_violations.is_empty()),
                Decision::Violation => assert_eq!(result.unwrap().rule_violations.len(), 1),
                Decision::Unknown => assert!(matches!(
                    result,
                    Err(AnalyzeError::Model(ModelError::Unknown(_)))
                )),
            }
        }
    }
}
#[test]
fn no_candidate_skips_model_and_missing_model_is_an_error() {
    let engine = select(&["prefer-functional-transforms".into()]).unwrap();
    assert!(
        Analyzer::analyze_source_with_rules("test.ts", "const f = xs => xs.map(x => x);", &engine)
            .unwrap()
            .rule_violations
            .is_empty()
    );
    assert!(matches!(
        Analyzer::analyze_source_with_rules(
            "test.ts",
            "function f(xs) { const a=[]; for (const x of xs) a.push(x); }",
            &engine
        ),
        Err(AnalyzeError::Model(ModelError::NotConfigured))
    ));
    assert!(
        !Analyzer::analyze_source_with_rules("test.ts", "const x: = 1;", &engine)
            .unwrap()
            .syntax_errors
            .is_empty()
    );
}
