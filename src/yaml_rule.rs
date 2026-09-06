//! Declarative call-expression rules. No script or shell execution.
use std::{collections::HashSet, fs, path::Path};

use oxc_ast::{
    AstKind,
    ast::{CallExpression, Expression, Program},
};
use oxc_ast_visit::{Visit, walk};
use oxc_span::{GetSpan, Span};
use serde::Deserialize;

use crate::{
    rule::{Rule, RuleContext},
    rules::no_set_state_in_effect::SetterCollector,
};

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
    #[serde(default = "version_one")]
    version: u32,
    id: String,
    message: String,
    #[serde(rename = "match")]
    selector: Selector,
    #[serde(rename = "where")]
    condition: Option<Condition>,
    judge: Option<Judge>,
}

fn version_one() -> u32 {
    1
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Selector {
    kind: NodeKind,
    callee: Option<Names>,
    #[serde(rename = "isStateSetter")]
    is_state_setter: Option<bool>,
}

#[derive(Debug, Deserialize)]
enum NodeKind {
    CallExpression,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Names {
    One(String),
    Many(Vec<String>),
}

impl Names {
    fn contains(&self, name: &str) -> bool {
        match self {
            Self::One(value) => value == name,
            Self::Many(values) => values.iter().any(|v| v == name),
        }
    }
    fn valid(&self) -> bool {
        let valid = |s: &str| !s.trim().is_empty();
        match self {
            Self::One(s) => valid(s),
            Self::Many(v) => !v.is_empty() && v.iter().all(|s| valid(s)),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Condition {
    callback: Option<Callback>,
    contains: Option<Selector>,
    all: Option<Vec<Condition>>,
    any: Option<Vec<Condition>>,
    not: Option<Box<Condition>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Callback {
    index: usize,
    contains: Selector,
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

impl YamlRule {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, YamlRuleError> {
        Self::parse(&fs::read_to_string(path)?)
    }

    pub fn parse(source: &str) -> Result<Self, YamlRuleError> {
        if source.len() > 256 * 1024 {
            return invalid("rule file exceeds 256 KiB");
        }
        let rule: Self = serde_yaml_ng::from_str(source)?;
        if rule.version != 1 {
            return invalid("only version 1 is supported");
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
        rule.selector.validate()?;
        if let Some(condition) = &rule.condition {
            condition.validate(0)?;
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
}

fn invalid<T>(message: &str) -> Result<T, YamlRuleError> {
    Err(YamlRuleError::Invalid(message.into()))
}

impl Selector {
    fn validate(&self) -> Result<(), YamlRuleError> {
        if self.callee.as_ref().is_some_and(|n| !n.valid()) {
            return invalid("callee must be a nonempty name or list of names");
        }
        Ok(())
    }

    fn matches(&self, call: &CallExpression<'_>, setters: &HashSet<String>) -> bool {
        match self.kind {
            NodeKind::CallExpression => {}
        }
        if let Some(names) = &self.callee
            && !callee_name(&call.callee).is_some_and(|name| names.contains(&name))
        {
            return false;
        }
        let is_setter = matches!(call.callee.get_inner_expression(), Expression::Identifier(id)
            if id.name == "setState" || setters.contains(id.name.as_str()));
        self.is_state_setter
            .is_none_or(|expected| expected == is_setter)
    }
}

fn callee_name(expression: &Expression<'_>) -> Option<String> {
    match expression.get_inner_expression() {
        Expression::Identifier(id) => Some(id.name.to_string()),
        Expression::StaticMemberExpression(member) => Some(format!(
            "{}.{}",
            callee_name(&member.object)?,
            member.property.name
        )),
        _ => None,
    }
}

impl Condition {
    fn validate(&self, depth: usize) -> Result<(), YamlRuleError> {
        if depth > 32 {
            return invalid("condition nesting exceeds 32 levels");
        }
        let operations = [
            self.callback.is_some(),
            self.contains.is_some(),
            self.all.is_some(),
            self.any.is_some(),
            self.not.is_some(),
        ];
        if operations.into_iter().filter(|v| *v).count() != 1 {
            return invalid(
                "each condition must have exactly one of callback, contains, all, any, not",
            );
        }
        if let Some(callback) = &self.callback {
            callback.contains.validate()?;
        }
        if let Some(selector) = &self.contains {
            selector.validate()?;
        }
        for conditions in [&self.all, &self.any].into_iter().flatten() {
            if conditions.is_empty() {
                return invalid("all/any must not be empty");
            }
            for condition in conditions {
                condition.validate(depth + 1)?;
            }
        }
        if let Some(condition) = &self.not {
            condition.validate(depth + 1)?;
        }
        Ok(())
    }

    fn matches(&self, call: &CallExpression<'_>, setters: &HashSet<String>) -> bool {
        if let Some(callback) = &self.callback {
            let Some(expression) = call
                .arguments
                .get(callback.index)
                .and_then(|arg| arg.as_expression())
            else {
                return false;
            };
            let expression = expression.get_inner_expression();
            if !matches!(
                expression,
                Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
            ) {
                return false;
            }
            let mut search = Contains {
                selector: &callback.contains,
                setters,
                found: false,
            };
            search.visit_expression(expression);
            return search.found;
        }
        if let Some(selector) = &self.contains {
            let mut search = Contains {
                selector,
                setters,
                found: false,
            };
            walk::walk_call_expression(&mut search, call);
            return search.found;
        }
        if let Some(conditions) = &self.all {
            return conditions.iter().all(|c| c.matches(call, setters));
        }
        if let Some(conditions) = &self.any {
            return conditions.iter().any(|c| c.matches(call, setters));
        }
        if let Some(condition) = &self.not {
            return !condition.matches(call, setters);
        }
        false
    }
}

struct Contains<'s> {
    selector: &'s Selector,
    setters: &'s HashSet<String>,
    found: bool,
}
impl<'a> Visit<'a> for Contains<'_> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if self.found {
            return;
        }
        self.found = self.selector.matches(call, self.setters);
        if !self.found {
            walk::walk_call_expression(self, call);
        }
    }
}

impl Rule for YamlRule {
    fn id(&self) -> &str {
        &self.id
    }

    fn check(&self, program: &Program<'_>, context: &mut RuleContext<'_>) {
        let mut setters = SetterCollector::default();
        setters.visit_program(program);
        YamlVisitor {
            rule: self,
            setters: &setters.names,
            context,
            source: program.source_text,
            functions: Vec::new(),
        }
        .visit_program(program);
    }
}

struct YamlVisitor<'r, 'out> {
    rule: &'r YamlRule,
    setters: &'r HashSet<String>,
    context: &'r mut RuleContext<'out>,
    source: &'r str,
    functions: Vec<Span>,
}

impl<'a> Visit<'a> for YamlVisitor<'_, '_> {
    fn enter_node(&mut self, kind: AstKind<'a>) {
        if matches!(
            kind,
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
        ) {
            self.functions.push(kind.span());
        }
    }
    fn leave_node(&mut self, kind: AstKind<'a>) {
        if matches!(
            kind,
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
        ) {
            self.functions.pop();
        }
    }
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if self.rule.selector.matches(call, self.setters)
            && self
                .rule
                .condition
                .as_ref()
                .is_none_or(|condition| condition.matches(call, self.setters))
        {
            if let Some(judge) = &self.rule.judge {
                let source = match judge.context {
                    ModelContext::SourceFile => self.source,
                    ModelContext::MatchedNode => {
                        &self.source[call.span.start as usize..call.span.end as usize]
                    }
                    ModelContext::EnclosingFunction => {
                        let span = self.functions.last().copied().unwrap_or(call.span);
                        &self.source[span.start as usize..span.end as usize]
                    }
                };
                self.context.request_model_with_message(
                    call.span,
                    &judge.criteria,
                    source,
                    &self.rule.message,
                );
            } else {
                self.context.report(call.span, &self.rule.message);
            }
        }
        walk::walk_call_expression(self, call);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        analyzer::{AnalyzeError, Analyzer},
        model::{Decision, ModelClient, ModelError, ModelJudgment, ModelRequest},
        rule_engine::RuleEngine,
    };

    const EFFECT: &str = include_str!("../rules/no-set-state-in-effect.yaml");

    fn check(yaml: &str, source: &str) -> Vec<crate::rule::RuleViolation> {
        let engine = RuleEngine::new(vec![Box::new(YamlRule::parse(yaml).unwrap())]);
        let result = Analyzer::analyze_source_with_rules("test.tsx", source, &engine).unwrap();
        assert!(
            result.syntax_errors.is_empty(),
            "{:?}",
            result.syntax_errors
        );
        result.rule_violations
    }

    #[test]
    fn yaml_effect_rule_matches_callbacks_and_reports_once_per_effect() {
        for source in [
            "useEffect(() => { setState(1); setState(2); }, []);",
            "React.useEffect(function () { if (ok) setState(1); }, []);",
            "const [x, update] = React.useState<number>(0); useEffect((() => update(1)), []);",
            "function App() { const [x, setX] = useState(0); useEffect(() => queueMicrotask(() => setX(1)), []); return <div />; }",
        ] {
            let violations = check(EFFECT, source);
            assert_eq!(violations.len(), 1, "{source}");
            assert_eq!(violations[0].rule_id, "no-set-state-in-effect");
            assert_eq!(
                violations[0].message,
                "useEffect에 setState를 넣어서는 안됩니다"
            );
        }
    }

    #[test]
    fn ignores_comments_strings_dependencies_and_other_calls() {
        for source in [
            "setState(1); useEffect(() => {}, []);",
            "useEffect(() => { /* setState(1) */ const text = 'setState(1)'; }, []);",
            "useEffect(() => console.log(setState), [setState(1)]);",
            "other.useEffect(() => setState(1));",
            "useEffect(callback, []);",
            "useEffect();",
        ] {
            assert!(check(EFFECT, source).is_empty(), "{source}");
        }
    }

    #[test]
    fn supports_nested_boolean_conditions_and_descendant_calls() {
        let yaml = "id: test\nmessage: found\nmatch: {kind: CallExpression, callee: outer}\nwhere:\n  all:\n    - any:\n        - contains: {kind: CallExpression, callee: good}\n        - contains: {kind: CallExpression, callee: alternate}\n    - not:\n        contains: {kind: CallExpression, callee: skip}\n";
        assert_eq!(
            check(
                yaml,
                "outer(good()); outer(alternate()); outer(skip(), good()); outer();"
            )
            .len(),
            2
        );
        let yaml = "id: test\nmessage: found\nmatch: {kind: CallExpression, callee: outer}\nwhere:\n  contains: {kind: CallExpression, callee: outer}\n";
        assert!(check(yaml, "outer();").is_empty());
        assert_eq!(check(yaml, "outer(outer());").len(), 1);
    }

    #[test]
    fn supports_callback_index_and_utf8_spans() {
        let yaml = EFFECT.replace("index: 0", "index: 1");
        assert_eq!(check(&yaml, "useEffect(0, () => setState(1));").len(), 1);
        assert!(check(&yaml, "useEffect(() => setState(1), []);").is_empty());
        let source = "const text = '한글'; useEffect(() => setState(1), []);";
        let span = check(EFFECT, source)[0].span;
        assert_eq!(
            &source[span.start as usize..span.end as usize],
            "useEffect(() => setState(1), [])"
        );
    }

    #[test]
    fn rejects_invalid_schemas_and_conditions() {
        for yaml in [
            EFFECT.replace("version: 1", "version: 2"),
            EFFECT.replace("CallExpression", "UnknownNode"),
            EFFECT.replace("isStateSetter", "isStateSettr"),
            EFFECT.replace("index: 0", "index: -1"),
            format!("{EFFECT}\nunknown: true"),
            "id: test\nmessage: ''\nmatch: {kind: CallExpression}".into(),
            "id: test\nmessage: found\nmatch: {kind: CallExpression, callee: []}".into(),
            "id: test\nmessage: found\nmatch: {kind: CallExpression}\nwhere: {all: []}".into(),
            "id: test\nmessage: found\nmatch: {kind: CallExpression}\nwhere: {}".into(),
            "id: test\nmessage: found\nmatch: {kind: CallExpression}\nwhere: {all: [], any: []}".into(),
            "id: test\nmessage: found\nmatch: {kind: CallExpression}\njudge: {criteria: x, context: unknown}".into(),
        ] { assert!(YamlRule::parse(&yaml).is_err(), "{yaml}"); }
    }

    struct Stub {
        decision: Decision,
        expected_source: String,
    }
    impl ModelClient for Stub {
        fn judge(&self, request: &ModelRequest) -> Result<ModelJudgment, ModelError> {
            assert_eq!(request.rule_id, "yaml/model");
            assert_eq!(request.criteria, "test criteria");
            assert_eq!(request.source, self.expected_source);
            Ok(ModelJudgment {
                decision: self.decision,
                reason: "model reason".into(),
            })
        }
    }

    #[test]
    fn model_context_and_decision_control_yaml_message() {
        let source = "const text = '한글'; function App() { target(); }";
        for (context, expected_source) in [
            ("matched_node", "target()"),
            ("enclosing_function", "function App() { target(); }"),
            ("source_file", source),
        ] {
            for decision in [Decision::Violation, Decision::Pass, Decision::Unknown] {
                let yaml = format!(
                    "id: yaml/model\nmessage: YAML message\nmatch: {{kind: CallExpression, callee: target}}\njudge: {{context: {context}, criteria: test criteria}}"
                );
                let engine = RuleEngine::new(vec![Box::new(YamlRule::parse(&yaml).unwrap())])
                    .with_model(Box::new(Stub {
                        decision,
                        expected_source: expected_source.into(),
                    }));
                let result = Analyzer::analyze_source_with_rules("test.ts", source, &engine);
                match decision {
                    Decision::Violation => {
                        let result = result.unwrap();
                        assert_eq!(result.rule_violations.len(), 1);
                        assert_eq!(result.rule_violations[0].message, "YAML message");
                    }
                    Decision::Pass => assert!(result.unwrap().rule_violations.is_empty()),
                    Decision::Unknown => assert!(matches!(
                        result,
                        Err(AnalyzeError::Model(ModelError::Unknown(_)))
                    )),
                }
            }
        }
    }

    #[test]
    fn model_required_only_for_matching_candidates() {
        let yaml = format!("{EFFECT}\njudge: {{criteria: test}}");
        let engine = RuleEngine::new(vec![Box::new(YamlRule::parse(&yaml).unwrap())]);
        assert!(
            Analyzer::analyze_source_with_rules("test.ts", "useEffect(() => {}, []);", &engine)
                .unwrap()
                .rule_violations
                .is_empty()
        );
        assert!(matches!(
            Analyzer::analyze_source_with_rules(
                "test.ts",
                "useEffect(() => setState(1), []);",
                &engine
            ),
            Err(AnalyzeError::Model(ModelError::NotConfigured))
        ));
    }
}
