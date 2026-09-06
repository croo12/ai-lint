//! Executes registered rules while the parsed AST is alive.

use crate::model::{Decision, ModelClient, ModelError, ModelRequest};
use crate::rule::{Rule, RuleContext, RuleViolation};
use oxc_ast::ast::Program;

#[derive(Default)]
pub struct RuleCheck {
    pub violations: Vec<RuleViolation>,
    pub model_requests: Vec<ModelRequest>,
}

#[derive(Default)]
pub struct RuleEngine {
    rules: Vec<Box<dyn Rule>>,
    model: Option<Box<dyn ModelClient>>,
}

impl RuleEngine {
    pub fn new(rules: Vec<Box<dyn Rule>>) -> Self {
        Self { rules, model: None }
    }

    pub fn with_model(mut self, model: Box<dyn ModelClient>) -> Self {
        self.model = Some(model);
        self
    }

    pub fn check(&self, program: &Program<'_>) -> RuleCheck {
        let mut check = RuleCheck::default();
        for rule in &self.rules {
            let mut context =
                RuleContext::new(rule.id(), &mut check.violations, &mut check.model_requests);
            rule.check(program, &mut context);
        }
        check
    }

    /// Sequential blocking inference; no network request is made for AST-only rules.
    pub fn resolve(&self, mut check: RuleCheck) -> Result<Vec<RuleViolation>, ModelError> {
        for request in check.model_requests {
            let judgment = self
                .model
                .as_ref()
                .ok_or(ModelError::NotConfigured)?
                .judge(&request)?;
            match judgment.decision {
                Decision::Violation => check.violations.push(RuleViolation {
                    rule_id: request.rule_id,
                    span: request.span,
                    message: request.message.unwrap_or(judgment.reason),
                }),
                Decision::Pass => {}
                Decision::Unknown => return Err(ModelError::Unknown(request.rule_id)),
            }
        }
        Ok(check.violations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::Analyzer;
    use oxc_ast::ast::Statement;

    // A test-only rule; this policy is not enabled in the CLI.
    struct NoTopLevelDebugger;

    impl Rule for NoTopLevelDebugger {
        fn id(&self) -> &'static str {
            "test/no-top-level-debugger"
        }

        fn check(&self, program: &Program<'_>, context: &mut RuleContext<'_>) {
            for statement in &program.body {
                if let Statement::DebuggerStatement(statement) = statement {
                    context.report(statement.span, "Unexpected debugger statement");
                }
            }
        }
    }

    #[test]
    fn reports_ast_matches_with_utf8_byte_offsets() {
        let source = "const text = '한글 debugger;'; debugger; debugger;";
        let engine = RuleEngine::new(vec![Box::new(NoTopLevelDebugger)]);
        let result = Analyzer::analyze_source_with_rules("test.ts", source, &engine).unwrap();

        assert!(result.syntax_errors.is_empty());
        assert_eq!(result.rule_violations.len(), 2);
        for violation in &result.rule_violations {
            assert_eq!(violation.rule_id, "test/no-top-level-debugger");
            assert_eq!(violation.message, "Unexpected debugger statement");
            assert_eq!(
                &source[violation.span.start as usize..violation.span.end as usize],
                "debugger;"
            );
        }
        assert!(result.rule_violations[0].span.start < result.rule_violations[1].span.start);
    }

    #[test]
    fn does_not_match_text_in_strings_or_comments() {
        let engine = RuleEngine::new(vec![Box::new(NoTopLevelDebugger)]);
        let result = Analyzer::analyze_source_with_rules(
            "test.js",
            "// debugger;\nconst text = 'debugger;';",
            &engine,
        )
        .unwrap();
        assert!(result.syntax_errors.is_empty());
        assert!(result.rule_violations.is_empty());
    }

    #[test]
    fn skips_rules_when_parsing_fails() {
        struct MustNotRun;
        impl Rule for MustNotRun {
            fn id(&self) -> &'static str {
                "test/must-not-run"
            }

            fn check(&self, _: &Program<'_>, _: &mut RuleContext<'_>) {
                panic!("rules must not run on invalid syntax");
            }
        }

        let engine = RuleEngine::new(vec![Box::new(MustNotRun)]);
        let result =
            Analyzer::analyze_source_with_rules("test.ts", "const x: = 1;", &engine).unwrap();
        assert!(!result.syntax_errors.is_empty());
        assert!(result.rule_violations.is_empty());
    }

    #[test]
    fn default_engine_has_no_rules() {
        let result =
            Analyzer::analyze_source_with_rules("test.js", "debugger;", &RuleEngine::default())
                .unwrap();
        assert!(result.syntax_errors.is_empty());
        assert!(result.rule_violations.is_empty());
    }

    struct ModelRule;
    impl Rule for ModelRule {
        fn id(&self) -> &'static str {
            "test/model"
        }
        fn check(&self, program: &Program<'_>, context: &mut RuleContext<'_>) {
            context.request_model(program.span, "Test criteria", program.source_text);
        }
    }

    struct StubModel(Decision);
    impl ModelClient for StubModel {
        fn judge(&self, request: &ModelRequest) -> Result<crate::model::ModelJudgment, ModelError> {
            assert_eq!(request.rule_id, "test/model");
            assert_eq!(request.criteria, "Test criteria");
            assert_eq!(request.source, "debugger;");
            Ok(crate::model::ModelJudgment {
                decision: self.0,
                reason: "model explanation".into(),
            })
        }
    }

    #[test]
    fn resolves_model_requests_alongside_static_findings() {
        for (decision, count) in [(Decision::Violation, 2), (Decision::Pass, 1)] {
            let engine = RuleEngine::new(vec![Box::new(NoTopLevelDebugger), Box::new(ModelRule)])
                .with_model(Box::new(StubModel(decision)));
            let result =
                Analyzer::analyze_source_with_rules("test.js", "debugger;", &engine).unwrap();
            assert_eq!(result.rule_violations.len(), count);
            if decision == Decision::Violation {
                assert_eq!(result.rule_violations[1].rule_id, "test/model");
                assert_eq!(result.rule_violations[1].message, "model explanation");
                assert_eq!(result.rule_violations[1].span, oxc_span::Span::new(0, 9));
            }
        }
    }

    #[test]
    fn missing_model_and_unknown_decisions_are_not_clean_results() {
        let engine = RuleEngine::new(vec![Box::new(ModelRule)]);
        assert!(matches!(
            Analyzer::analyze_source_with_rules("test.js", "debugger;", &engine),
            Err(crate::analyzer::AnalyzeError::Model(
                ModelError::NotConfigured
            ))
        ));
        let engine = engine.with_model(Box::new(StubModel(Decision::Unknown)));
        assert!(matches!(
            Analyzer::analyze_source_with_rules("test.js", "debugger;", &engine),
            Err(crate::analyzer::AnalyzeError::Model(ModelError::Unknown(id))) if id == "test/model"
        ));
    }

    #[test]
    fn syntax_errors_skip_model_requests() {
        let engine = RuleEngine::new(vec![Box::new(ModelRule)]);
        let result =
            Analyzer::analyze_source_with_rules("test.ts", "const x: = 1;", &engine).unwrap();
        assert!(!result.syntax_errors.is_empty());
    }
}
