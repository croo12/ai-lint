//! Explicit opt-in example: sends the input file to the configured server.
use std::{env, error::Error, process::ExitCode};

use ai_lint::{
    analyzer::Analyzer,
    model::{ModelConfig, ModelError, OpenAiCompatibleClient},
    rule::{Rule, RuleContext},
    rule_engine::RuleEngine,
};
use oxc_ast::ast::Program;

struct ContextualRule {
    criteria: String,
}

impl Rule for ContextualRule {
    fn id(&self) -> &'static str {
        "example/contextual-rule"
    }

    fn check(&self, program: &Program<'_>, context: &mut RuleContext<'_>) {
        context.request_model(program.span, &self.criteria, program.source_text);
    }
}

fn run() -> Result<ExitCode, Box<dyn Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: cargo run --example model_rule -- FILE CRITERIA".into());
    }
    let config = ModelConfig::load(".env")?.ok_or(ModelError::NotConfigured)?;
    let engine = RuleEngine::new(vec![Box::new(ContextualRule {
        criteria: args[1].clone(),
    })])
    .with_model(Box::new(OpenAiCompatibleClient::new(config)?));
    let analyzed = Analyzer::analyze_file_with_rules(args[0].as_ref(), &engine)?;
    for error in &analyzed.syntax_errors {
        eprintln!("{error}");
    }
    for violation in &analyzed.rule_violations {
        eprintln!("[{}] {}", violation.rule_id, violation.message);
    }
    let has_findings = !analyzed.syntax_errors.is_empty() || !analyzed.rule_violations.is_empty();
    if !has_findings {
        println!("No findings");
    }
    Ok(ExitCode::from(u8::from(has_findings)))
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("ai-lint: {error}");
            ExitCode::from(2)
        }
    }
}
