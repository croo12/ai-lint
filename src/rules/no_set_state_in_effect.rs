//! Syntactic check for setter calls inside inline useEffect callbacks.
//!
//! Recognizes useEffect/React.useEffect, literal setState calls, and setter
//! names destructured from useState/React.useState in the same file. Import
//! aliases, binding shadowing, and callbacks passed by reference are not resolved.

use std::collections::HashSet;

use oxc_ast::ast::{BindingPattern, CallExpression, Expression, Program, VariableDeclarator};
use oxc_ast_visit::{Visit, walk};

use crate::rule::{Rule, RuleContext};

pub struct NoSetStateInEffect;

impl Rule for NoSetStateInEffect {
    fn id(&self) -> &'static str {
        "no-set-state-in-effect"
    }

    fn check(&self, program: &Program<'_>, context: &mut RuleContext<'_>) {
        let mut setters = SetterCollector::default();
        setters.visit_program(program);
        EffectVisitor {
            setters: &setters.names,
            context,
            effect_depth: 0,
        }
        .visit_program(program);
    }
}

fn is_hook(expression: &Expression<'_>, name: &str) -> bool {
    match expression.get_inner_expression() {
        Expression::Identifier(identifier) => identifier.name == name,
        Expression::StaticMemberExpression(member) => {
            matches!(member.object.get_inner_expression(), Expression::Identifier(object) if object.name == "React")
                && member.property.name == name
        }
        _ => false,
    }
}

#[derive(Default)]
pub(crate) struct SetterCollector {
    pub(crate) names: HashSet<String>,
}

impl<'a> Visit<'a> for SetterCollector {
    fn visit_variable_declarator(&mut self, declaration: &VariableDeclarator<'a>) {
        if let BindingPattern::ArrayPattern(pattern) = &declaration.id
            && let Some(initializer) = &declaration.init
            && let Expression::CallExpression(call) = initializer.get_inner_expression()
            && is_hook(&call.callee, "useState")
            && let Some(Some(BindingPattern::BindingIdentifier(setter))) = pattern.elements.get(1)
        {
            self.names.insert(setter.name.to_string());
        }
        walk::walk_variable_declarator(self, declaration);
    }
}

struct EffectVisitor<'ctx, 'out> {
    setters: &'ctx HashSet<String>,
    context: &'ctx mut RuleContext<'out>,
    effect_depth: usize,
}

impl<'a> Visit<'a> for EffectVisitor<'_, '_> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if self.effect_depth > 0
            && let Expression::Identifier(callee) = call.callee.get_inner_expression()
            && (callee.name == "setState" || self.setters.contains(callee.name.as_str()))
        {
            self.context
                .report(call.span, "useEffect에 setState를 넣어서는 안됩니다");
        }

        if is_hook(&call.callee, "useEffect") {
            self.visit_expression(&call.callee);
            for (index, argument) in call.arguments.iter().enumerate() {
                let is_callback = index == 0
                    && argument.as_expression().is_some_and(|expression| {
                        matches!(
                            expression.get_inner_expression(),
                            Expression::ArrowFunctionExpression(_)
                                | Expression::FunctionExpression(_)
                        )
                    });
                if is_callback {
                    self.effect_depth += 1;
                }
                self.visit_argument(argument);
                if is_callback {
                    self.effect_depth -= 1;
                }
            }
        } else {
            walk::walk_call_expression(self, call);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::analyzer::Analyzer;

    fn findings(source: &str) -> usize {
        let result = Analyzer::analyze_source("test.tsx", source).unwrap();
        assert!(
            result.syntax_errors.is_empty(),
            "{:?}",
            result.syntax_errors
        );
        for violation in &result.rule_violations {
            assert_eq!(violation.rule_id, "no-set-state-in-effect");
            assert_eq!(
                violation.message,
                "useEffect에 setState를 넣어서는 안됩니다"
            );
        }
        result.rule_violations.len()
    }

    #[test]
    fn detects_setters_in_inline_callbacks() {
        for source in [
            "useEffect(() => { setState(1); }, []);",
            "useEffect(() => setState(1), []);",
            "React.useEffect(function () { if (ready) setState(1); }, []);",
            "useEffect((() => { setState(1); }), []);",
            "function App() { const [count, setCount] = useState(0); useEffect(() => { setCount(1); }, []); return <div>{count}</div>; }",
            "const [value, update] = React.useState<number>(0); React.useEffect(() => update(2), []);",
        ] {
            assert_eq!(findings(source), 1, "{source}");
        }
    }

    #[test]
    fn ignores_non_calls_and_calls_outside_effect_callbacks() {
        for source in [
            "setState(1); useEffect(() => console.log('hello'), []);",
            "useEffect(() => { /* setState(1) */ const text = 'setState(1)'; }, []);",
            "useEffect(() => console.log(setState), [setState]);",
            "useEffect(() => {}, [setState(1)]);",
            "otherHook(() => setState(1));",
            "useEffect(() => setTimeout(() => {}, 10), []);",
            "service.useEffect(() => setState(1));",
        ] {
            assert_eq!(findings(source), 0, "{source}");
        }
    }

    #[test]
    fn reports_each_call_once_including_nested_callbacks() {
        assert_eq!(
            findings(
                "useEffect(() => { setState(1); useEffect(() => setState(2), []); queueMicrotask(() => setState(3)); }, []);"
            ),
            3
        );
    }

    #[test]
    fn reports_the_setter_call_span() {
        let source = "useEffect(() => { const text = '한글'; setState(1); }, []);";
        let result = Analyzer::analyze_source("test.ts", source).unwrap();
        let span = result.rule_violations[0].span;
        assert_eq!(
            &source[span.start as usize..span.end as usize],
            "setState(1)"
        );
    }
}
