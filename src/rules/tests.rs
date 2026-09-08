use super::*;
use crate::{
    analyzer::{AnalyzeError, Analyzer},
    model::{Decision, ModelClient, ModelError, ModelJudgment, ModelRequest},
    rule::RuleViolation,
};
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
