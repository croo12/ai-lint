//! Explicit opt-in example: a compiled rule sends candidate excerpts to the model.
use std::{env, error::Error, process::ExitCode};

use ai_lint::{
    analyzer::{Allocator, Analyzer, ChangedFile},
    model::{ModelConfig, ModelError, OpenAiCompatibleClient},
    rules,
};

fn run() -> Result<ExitCode, Box<dyn Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: cargo run --example model_rule -- FILE RULE_ID".into());
    }
    let config = ModelConfig::load(".env")?.ok_or(ModelError::NotConfigured)?;
    let engine = rules::select(&[args[1].clone()])?
        .with_model(Box::new(OpenAiCompatibleClient::new(config)?));
    let file = ChangedFile::read(&args[0])?;
    let (syntax_errors, check) = {
        let allocator = Allocator::default();
        let analyzed = Analyzer::analyze(&allocator, &file)?;
        let check = engine.check((&analyzed).into())?;
        (analyzed.syntax_errors, check)
    };
    let violations = engine.resolve(check)?;
    for error in &syntax_errors {
        eprintln!("{error}");
    }
    for violation in &violations {
        eprintln!("[{}] {}", violation.rule_id, violation.message);
    }
    let has_findings = !syntax_errors.is_empty() || !violations.is_empty();
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
