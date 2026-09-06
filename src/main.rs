use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use ai_lint::analyzer::Analyzer;
use ai_lint::{
    model::{ModelConfig, OpenAiCompatibleClient},
    rule_engine::RuleEngine,
    yaml_rule,
};
use clap::{Args, Parser, Subcommand};

const EXIT_CLEAN: u8 = 0;
const EXIT_FINDINGS: u8 = 1;
const EXIT_ERROR: u8 = 2;

#[derive(Debug, Parser)]
#[command(name = "ai-lint", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Parse changed source files and check them with AI Lint.
    Check(CheckArgs),
}

#[derive(Debug, Args)]
struct CheckArgs {
    /// TypeScript or JavaScript source files to check.
    #[arg(value_name = "FILE", required = true)]
    files: Vec<PathBuf>,
    /// Model configuration file. Process environment takes precedence.
    #[arg(long, default_value = ".env")]
    env_file: PathBuf,
    /// YAML rule file (repeatable). When provided, replaces built-in rules.
    #[arg(long = "rules", value_name = "YAML")]
    rule_files: Vec<PathBuf>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Command::Check(args) => run_check(args),
    }
}

fn run_check(args: CheckArgs) -> ExitCode {
    let engine = match configured_engine(&args.env_file, &args.rule_files) {
        Ok(engine) => engine,
        Err(error) => {
            eprintln!("ai-lint: {error}");
            return ExitCode::from(EXIT_ERROR);
        }
    };
    let mut finding_count = 0;

    for path in &args.files {
        match check_file(path, &engine) {
            Ok(count) => finding_count += count,
            Err(message) => {
                eprintln!("ai-lint: {message}");
                return ExitCode::from(EXIT_ERROR);
            }
        }
    }

    if finding_count == 0 {
        println!("Checked {} file(s): no findings", args.files.len());
        ExitCode::from(EXIT_CLEAN)
    } else {
        eprintln!("Found {finding_count} finding(s)");
        ExitCode::from(EXIT_FINDINGS)
    }
}

fn configured_engine(
    env_file: &Path,
    rule_files: &[PathBuf],
) -> Result<RuleEngine, Box<dyn std::error::Error>> {
    let engine = if rule_files.is_empty() {
        yaml_rule::default_engine()
    } else {
        let mut loaded: Vec<Box<dyn ai_lint::rule::Rule>> = Vec::new();
        let mut ids = std::collections::HashSet::new();
        for path in rule_files {
            let rule = ai_lint::yaml_rule::YamlRule::load(path)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            let id = ai_lint::rule::Rule::id(&rule).to_owned();
            if !ids.insert(id.clone()) {
                return Err(format!("{}: duplicate rule id: {id}", path.display()).into());
            }
            loaded.push(Box::new(rule));
        }
        RuleEngine::new(loaded)
    };
    match ModelConfig::load(env_file)? {
        Some(config) => Ok(engine.with_model(Box::new(OpenAiCompatibleClient::new(config)?))),
        None => Ok(engine),
    }
}

fn check_file(path: &Path, engine: &RuleEngine) -> Result<usize, String> {
    let analyzed = Analyzer::analyze_file_with_rules(path, engine)
        .map_err(|error| format!("{}: {error}", path.display()))?;

    for diagnostic in &analyzed.syntax_errors {
        eprintln!("{}: {diagnostic}", path.display());
    }

    for violation in &analyzed.rule_violations {
        eprintln!(
            "{}:{}..{}: [{}] {}",
            path.display(),
            violation.span.start,
            violation.span.end,
            violation.rule_id,
            violation.message,
        );
    }

    Ok(analyzed.syntax_errors.len() + analyzed.rule_violations.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_typescript() {
        let analyzed = Analyzer::analyze_source("test.ts", "const greeting: string = 'hello';")
            .expect("valid TypeScript should be analyzed");

        assert!(analyzed.syntax_errors.is_empty());
        assert!(analyzed.ast_debug.contains("VariableDeclaration"));
    }

    #[test]
    fn reports_invalid_typescript() {
        let analyzed = Analyzer::analyze_source("test.ts", "const greeting: = 'hello';")
            .expect("invalid TypeScript should still be analyzed");

        assert!(!analyzed.syntax_errors.is_empty());
    }
}
