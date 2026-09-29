use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use ai_lint::{
    analyzer::{Allocator, Analyzer, ChangedFile},
    pipeline::rule_input,
    rule_engine::RuleEngine,
    rules::{
        self,
        ai_model::{ModelClient, ModelConfig, OpenAiCompatibleClient, evaluate_reviews},
        contract::RuleViolation,
    },
};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde_json::json;

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
enum OutputFormat {
    Text,
    Json,
}

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
    /// List compiled rule IDs.
    Rules,
}

#[derive(Debug, Args)]
struct CheckArgs {
    /// TypeScript or JavaScript source files to check.
    #[arg(value_name = "FILE", required = true)]
    files: Vec<PathBuf>,
    /// Model configuration file. Process environment takes precedence.
    #[arg(long, default_value = ".env")]
    env_file: PathBuf,
    /// Compiled rule ID (repeatable). Replaces the default selection.
    #[arg(long = "rules", value_name = "ID")]
    rule_ids: Vec<String>,
    /// Output format for command-line use or integrations.
    #[arg(long, value_enum, default_value = "text")]
    format: OutputFormat,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Command::Check(args) => run_check(args),
        Command::Rules => {
            println!("{}", rules::IDS.join("\n"));
            ExitCode::SUCCESS
        }
    }
}

fn run_check(args: CheckArgs) -> ExitCode {
    if args.format == OutputFormat::Json {
        return run_check_json(args);
    }
    let services = match configured_services(&args.env_file, &args.rule_ids) {
        Ok(services) => services,
        Err(error) => {
            eprintln!("ai-lint: {error}");
            return ExitCode::from(EXIT_ERROR);
        }
    };
    let mut finding_count = 0;

    for path in &args.files {
        match check_file(path, &services) {
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

fn run_check_json(args: CheckArgs) -> ExitCode {
    let mut files = Vec::new();
    let mut errors = Vec::new();
    let mut findings = 0;

    match configured_services(&args.env_file, &args.rule_ids) {
        Err(error) => errors.push(error.to_string()),
        Ok(services) => {
            for path in &args.files {
                match analyze_and_check_file(path, &services) {
                    Err(error) => errors.push(format!("{}: {error}", path.display())),
                    Ok(analyzed) => {
                        findings += analyzed.syntax_errors.len() + analyzed.rule_violations.len();
                        let violations: Vec<_> = analyzed.rule_violations.iter().map(|v| {
                            let prefix = analyzed.source.get(..v.span.start as usize).unwrap_or("");
                            let line = prefix.bytes().filter(|c| *c == b'\n').count() + 1;
                            let column = prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1;
                            json!({"ruleId": v.rule_id, "message": v.message, "start": v.span.start,
                                "end": v.span.end, "line": line, "column": column})
                        }).collect();

                        files.push(json!({"path": path, "syntaxErrors": analyzed.syntax_errors, "violations": violations}));
                    }
                }
            }
        }
    }

    let exit_code = if !errors.is_empty() {
        EXIT_ERROR
    } else if findings > 0 {
        EXIT_FINDINGS
    } else {
        EXIT_CLEAN
    };

    println!(
        "{}",
        json!({"schemaVersion": 1, "exitCode": exit_code, "files": files, "errors": errors})
    );

    ExitCode::from(exit_code)
}

struct CheckServices {
    engine: RuleEngine,
    model: Option<OpenAiCompatibleClient>,
}

fn configured_services(
    env_file: &Path,
    rule_ids: &[String],
) -> Result<CheckServices, Box<dyn std::error::Error>> {
    let selected_rules = if rule_ids.is_empty() {
        rules::default_rules()
    } else {
        rules::select(rule_ids)?
    };
    let model = ModelConfig::load(env_file)?
        .map(OpenAiCompatibleClient::new)
        .transpose()?;
    Ok(CheckServices {
        engine: RuleEngine::new(selected_rules),
        model,
    })
}

fn check_file(path: &Path, services: &CheckServices) -> Result<usize, String> {
    let analyzed = analyze_and_check_file(path, services)
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

struct CheckedFile {
    source: String,
    syntax_errors: Vec<String>,
    rule_violations: Vec<RuleViolation>,
}

/// The CLI composes analysis and evaluation, retaining only owned data for output.
fn analyze_and_check_file(path: &Path, services: &CheckServices) -> Result<CheckedFile, String> {
    let file = ChangedFile::read(path).map_err(|error| error.to_string())?;
    let (syntax_errors, check) = {
        let allocator = Allocator::default();
        let analyzed = Analyzer::analyze(&allocator, &file).map_err(|error| error.to_string())?;
        let check = services
            .engine
            .check(rule_input(&analyzed))
            .map_err(|error| format!("rule evaluation failed: {error}"))?;
        (analyzed.syntax_errors, check)
    };
    // No AST or allocator is retained while the model processes owned requests.
    let mut rule_violations = check.violations;
    rule_violations.extend(
        evaluate_reviews(
            check.reviews,
            services
                .model
                .as_ref()
                .map(|model| model as &dyn ModelClient),
        )
        .map_err(|error| format!("model evaluation failed: {error}"))?,
    );
    Ok(CheckedFile {
        source: file.source,
        syntax_errors,
        rule_violations,
    })
}
