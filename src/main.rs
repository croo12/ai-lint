use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

mod analyzer;

use analyzer::Analyzer;
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
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Command::Check(args) => run_check(args),
    }
}

fn run_check(args: CheckArgs) -> ExitCode {
    let mut finding_count = 0;

    for path in &args.files {
        match check_file(path) {
            Ok(count) => finding_count += count,
            Err(message) => {
                eprintln!("ai-lint: {message}");
                return ExitCode::from(EXIT_ERROR);
            }
        }
    }

    if finding_count == 0 {
        println!("Checked {} file(s): no syntax errors", args.files.len());
        ExitCode::from(EXIT_CLEAN)
    } else {
        eprintln!("Found {finding_count} syntax error(s)");
        ExitCode::from(EXIT_FINDINGS)
    }
}

fn check_file(path: &Path) -> Result<usize, String> {
    let analyzed =
        Analyzer::analyze_file(path).map_err(|error| format!("{}: {error}", path.display()))?;

    for diagnostic in &analyzed.syntax_errors {
        eprintln!("{}: {diagnostic}", path.display());
    }

    Ok(analyzed.syntax_errors.len())
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
