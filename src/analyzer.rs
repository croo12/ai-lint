use std::{
    fmt, fs,
    path::{Path, PathBuf},
};

use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::model::ModelError;
use crate::{rule::RuleViolation, rule_engine::RuleEngine, yaml_rule};

/// Owned result of analyzing one source file.
///
/// Oxc's AST borrows its allocator and the source text, so this module converts
/// it to an owned representation before returning it to callers.
#[derive(Debug, Clone)]
pub struct AnalyzedFile {
    pub path: PathBuf,
    pub source: String,
    pub ast_debug: String,
    pub syntax_errors: Vec<String>,
    pub rule_violations: Vec<RuleViolation>,
}

#[derive(Debug)]
pub enum AnalyzeError {
    Read(std::io::Error),
    UnsupportedSource,
    Model(ModelError),
}

impl fmt::Display for AnalyzeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(formatter, "failed to read source: {error}"),
            Self::Model(error) => write!(formatter, "model evaluation failed: {error}"),
            Self::UnsupportedSource => {
                write!(formatter, "expected a JavaScript or TypeScript file")
            }
        }
    }
}

impl std::error::Error for AnalyzeError {}

pub struct Analyzer;

impl Analyzer {
    pub fn analyze_file(path: &Path) -> Result<AnalyzedFile, AnalyzeError> {
        Self::analyze_file_with_rules(path, &yaml_rule::default_engine())
    }

    pub fn analyze_file_with_rules(
        path: &Path,
        rules: &RuleEngine,
    ) -> Result<AnalyzedFile, AnalyzeError> {
        let source = fs::read_to_string(path).map_err(AnalyzeError::Read)?;
        Self::analyze_source_path(path, source, rules)
    }

    pub fn analyze_source(
        file_name: impl AsRef<Path>,
        source: impl Into<String>,
    ) -> Result<AnalyzedFile, AnalyzeError> {
        Self::analyze_source_with_rules(file_name, source, &yaml_rule::default_engine())
    }

    pub fn analyze_source_with_rules(
        file_name: impl AsRef<Path>,
        source: impl Into<String>,
        rules: &RuleEngine,
    ) -> Result<AnalyzedFile, AnalyzeError> {
        Self::analyze_source_path(file_name.as_ref(), source.into(), rules)
    }

    fn analyze_source_path(
        path: &Path,
        source: String,
        rules: &RuleEngine,
    ) -> Result<AnalyzedFile, AnalyzeError> {
        let source_type =
            SourceType::from_path(path).map_err(|_| AnalyzeError::UnsupportedSource)?;
        let allocator = Allocator::default();
        let result = Parser::new(&allocator, &source, source_type).parse();

        let syntax_errors: Vec<String> = result
            .diagnostics
            .iter()
            .map(|diagnostic| format!("{diagnostic:?}"))
            .collect();

        let ast_debug = format!("{:#?}", result.program);

        // Rules only see a successfully parsed program, never a recovered AST.
        let check = if syntax_errors.is_empty() && !result.panicked {
            rules.check(&result.program)
        } else {
            Default::default()
        };
        drop(result);
        drop(allocator);
        let rule_violations = rules.resolve(check).map_err(AnalyzeError::Model)?;

        Ok(AnalyzedFile {
            path: path.to_path_buf(),
            source,
            ast_debug,
            syntax_errors,
            rule_violations,
        })
    }
}
