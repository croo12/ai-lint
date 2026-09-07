use super::*;
use crate::{
    analyzer::{AnalyzeError, Analyzer},
    model::{Decision, ModelClient, ModelError, ModelJudgment, ModelRequest},
    rule::RuleViolation,
};
const EFFECT: &str = include_str!("../../rules/no-set-state-in-effect.yaml");
const COLLECTION: &str = include_str!("../../rules/examples/prefer-functional-transforms.yaml");

fn check(yaml: &str, source: &str) -> Vec<RuleViolation> {
    let engine = RuleEngine::new(vec![YamlRule::parse(yaml).unwrap()]);
    let result = Analyzer::analyze_source_with_rules("test.tsx", source, &engine).unwrap();
    assert!(
        result.syntax_errors.is_empty(),
        "{:?}",
        result.syntax_errors
    );
    result.rule_violations
}
fn collection() -> &'static str {
    COLLECTION.split("judge:").next().unwrap()
}

#[test]
fn collection_yaml_matches_loop_forms_and_function_forms_once() {
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
            assert_eq!(check(collection(), &source).len(), 1, "{source}");
        }
    }
}

#[test]
fn collection_yaml_excludes_non_candidates_and_shadowed_bindings() {
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
        assert!(check(collection(), source).is_empty(), "{source}");
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
        assert_eq!(check(collection(), source).len(), 1, "{source}");
    }
}

#[test]
fn nested_functions_and_utf8_spans_are_independent() {
    let inner =
        "function inner(xs) { const out = []; for (const x of xs) out.push(x); return out; }";
    let source = format!("const label = '한글'; function outer() {{ {inner} }}");
    let found = check(collection(), &source);
    assert_eq!(found.len(), 1);
    assert_eq!(
        &source[found[0].span.start as usize..found[0].span.end as usize],
        inner
    );
}

#[test]
fn effect_policy_is_entirely_yaml_and_preserves_existing_cases() {
    for source in [
        "useEffect(() => { setState(1); setState(2); }, []);",
        "React.useEffect(function () { if (ok) setState(1); }, []);",
        "const [x, update] = React.useState<number>(0); useEffect((() => update(1)), []);",
        "function App() { const [x, setX] = useState(0); useEffect(() => queueMicrotask(() => setX(1)), []); return <div />; }",
    ] {
        assert_eq!(check(EFFECT, source).len(), 1, "{source}");
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
        assert!(check(EFFECT, source).is_empty(), "{source}");
    }
}

#[test]
fn generic_queries_support_fields_children_ancestors_boolean_logic_and_captures() {
    let yaml = "version: 2\nid: no-debug\nmessage: debug\nmatch:\n  kind: FunctionDeclaration\n  capture: owner\nwhere:\n  at:\n    path: body\n    match:\n      child:\n        match:\n          kind: DebuggerStatement\n          ancestor:\n            match:\n              kind: FunctionDeclaration\n              at:\n                path: $owner.id\n                match: {properties: {name: target}}";
    assert_eq!(
        check(
            yaml,
            "function target() { debugger; debugger; } function other() { debugger; }"
        )
        .len(),
        1
    );
    assert!(check(yaml, "function target() { if (ok) { debugger; } }").is_empty());
    let yaml = "version: 2\nid: calls\nmessage: found\nmatch: {kind: CallExpression}\nwhere:\n  all:\n    - any:\n        - properties: {callee.name: first}\n        - properties: {callee.name: second}\n    - not:\n        descendant:\n          match: {kind: CallExpression, properties: {callee.name: skip}}";
    assert_eq!(
        check(yaml, "first(); second(); third(); first(skip());").len(),
        2
    );
}

#[test]
fn unresolved_identifiers_are_not_equal_bindings() {
    let yaml = "version: 2\nid: binding\nmessage: same\nmatch: {kind: CallExpression}\nwhere: {same_binding: {left: callee, right: arguments.0}}";
    assert!(check(yaml, "missing(missing);").is_empty());
    assert_eq!(
        check(yaml, "const local = () => {}; local(local);").len(),
        1
    );
}

#[test]
fn validates_schema_and_capture_flow_before_execution() {
    for body in [
        "match: {}",
        "match: {kind: []}",
        "match: {kind: Function, hasLoopAccumulator: true}",
        "match: {kind: CallExpression, isStateSetter: true}",
        "match: {all: []}",
        "match: {any: []}",
        "match: {kind: Identifier, exists: [foo..bar]}",
        "match: {same_binding: {left: '.', right: '$missing'}}",
        "match: {kind: Identifier, capture: x}\nwhere: {capture: x}",
        "match: {not: {capture: x}}\nwhere: {at: {path: '$x', match: {kind: Identifier}}}",
        "match: {any: [{capture: x}, {kind: Identifier}]}\nwhere: {at: {path: '$x', match: {kind: Identifier}}}",
        "match: {child: {stop_at: Identifier, match: {kind: Identifier}}}",
        "match: {kind: Program}\njudge: {criteria: ''}",
    ] {
        let yaml = format!("version: 2\nid: test\nmessage: test\n{body}");
        assert!(YamlRule::parse(&yaml).is_err(), "{yaml}");
    }
    assert!(
        YamlRule::parse("version: 1\nid: old\nmessage: old\nmatch: {kind: CallExpression}")
            .is_err()
    );
    assert!(YamlRule::parse(&" ".repeat(256 * 1024 + 1)).is_err());
}

struct Stub {
    decision: Decision,
    expected: String,
}
impl ModelClient for Stub {
    fn judge(&self, request: &ModelRequest) -> Result<ModelJudgment, ModelError> {
        assert_eq!(request.source, self.expected);
        Ok(ModelJudgment {
            decision: self.decision,
            reason: "model reason".into(),
        })
    }
}

#[test]
fn model_contexts_and_decisions_are_preserved() {
    let function = "function target() { go(); }";
    let source = format!("const label = '한글'; {function}");
    for (context, expected) in [
        ("matched_node", "go()"),
        ("enclosing_function", function),
        ("source_file", source.as_str()),
    ] {
        for decision in [Decision::Pass, Decision::Violation, Decision::Unknown] {
            let yaml = format!(
                "version: 2\nid: model\nmessage: YAML message\nmatch: {{kind: CallExpression}}\njudge: {{context: {context}, criteria: check}}"
            );
            let engine =
                RuleEngine::new(vec![YamlRule::parse(&yaml).unwrap()]).with_model(Box::new(Stub {
                    decision,
                    expected: expected.into(),
                }));
            let result = Analyzer::analyze_source_with_rules("test.ts", &source, &engine);
            match decision {
                Decision::Pass => assert!(result.unwrap().rule_violations.is_empty()),
                Decision::Violation => {
                    let found = result.unwrap().rule_violations;
                    assert_eq!(found.len(), 1);
                    assert_eq!(found[0].message, "YAML message");
                }
                Decision::Unknown => assert!(matches!(
                    result,
                    Err(AnalyzeError::Model(ModelError::Unknown(_)))
                )),
            }
        }
    }
}

#[test]
fn collection_queues_one_model_request_and_skips_non_candidates() {
    let engine = RuleEngine::new(vec![YamlRule::parse(COLLECTION).unwrap()]);
    assert!(
        Analyzer::analyze_source_with_rules("test.ts", "const f = xs => xs.map(x => x);", &engine)
            .unwrap()
            .rule_violations
            .is_empty()
    );
    let source = "function f(xs) { const a = []; const b = []; for (const x of xs) { a.push(x); b.push(x); } return a; }";
    assert!(matches!(
        Analyzer::analyze_source_with_rules("test.ts", source, &engine),
        Err(AnalyzeError::Model(ModelError::NotConfigured))
    ));
    let engine = engine.with_model(Box::new(Stub {
        decision: Decision::Violation,
        expected: source.into(),
    }));
    assert_eq!(
        Analyzer::analyze_source_with_rules("test.ts", source, &engine)
            .unwrap()
            .rule_violations
            .len(),
        1
    );
    assert!(
        !Analyzer::analyze_source_with_rules("test.ts", "const x: = 1;", &engine)
            .unwrap()
            .syntax_errors
            .is_empty()
    );
}

#[test]
fn budget_exhaustion_is_an_error() {
    let tree = serde_json::json!({"type":"Program","start":0,"end":0});
    let doc = Document::new("", &tree, Default::default());
    let query: Query = serde_yaml_ng::from_str("kind: Program").unwrap();
    assert!(query.evaluate(&doc, 0, Default::default(), &mut 0).is_err());
}
