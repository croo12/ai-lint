//! Parses changed files and describes their changes, independently of rules and models.
//!
//! The caller owns the source and allocator so the returned AST can be consumed
//! directly without serializing it or parsing the source again.

mod change;

pub use change::{ChangeKind, ChangeMetadata, ChangedRange, PreviousSource};
pub use oxc_allocator::Allocator;

use std::{
    fmt, fs,
    path::{Path, PathBuf},
};

use oxc_ast::ast::Program;
use oxc_parser::{ParseOptions, Parser};
use oxc_span::SourceType;

/// A current source snapshot, optionally paired with its previous version.
/// Selecting changed files (for example, using Git) is the caller's responsibility.
#[derive(Debug, Clone)]
pub struct ChangedFile {
    pub path: PathBuf,
    pub source: String,
    pub previous: PreviousSource,
}

impl ChangedFile {
    /// Without a baseline, change metadata is explicitly marked as unknown.
    pub fn new(path: impl Into<PathBuf>, source: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            source: source.into(),
            previous: PreviousSource::Unknown,
        }
    }

    pub fn read(path: impl AsRef<Path>) -> Result<Self, AnalyzeError> {
        let path = path.as_ref();
        let source = fs::read_to_string(path).map_err(AnalyzeError::Read)?;
        Ok(Self::new(path, source))
    }

    pub fn with_previous_source(mut self, source: impl Into<String>) -> Self {
        self.previous = PreviousSource::Present(source.into());
        self
    }

    pub fn added(path: impl Into<PathBuf>, source: impl Into<String>) -> Self {
        Self {
            previous: PreviousSource::Absent,
            ..Self::new(path, source)
        }
    }
}

/// The AST borrows the caller's allocator and source snapshot.
#[derive(Debug)]
pub struct AnalyzedFile<'a> {
    pub path: &'a Path,
    pub source: &'a str,
    pub ast: Program<'a>,
    pub changes: ChangeMetadata,
    pub syntax_errors: Vec<String>,
    pub panicked: bool,
}

impl AnalyzedFile<'_> {
    /// A recovered or aborted parse must not be used for rule evaluation.
    pub fn is_valid(&self) -> bool {
        self.syntax_errors.is_empty() && !self.panicked
    }
}

#[derive(Debug)]
pub enum AnalyzeError {
    Read(std::io::Error),
    UnsupportedSource,
}

impl fmt::Display for AnalyzeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(formatter, "failed to read source: {error}"),
            Self::UnsupportedSource => {
                write!(formatter, "expected a JavaScript or TypeScript file")
            }
        }
    }
}

impl std::error::Error for AnalyzeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(error) => Some(error),
            Self::UnsupportedSource => None,
        }
    }
}

pub struct Analyzer;

impl Analyzer {
    pub fn analyze<'a>(
        allocator: &'a Allocator,
        file: &'a ChangedFile,
    ) -> Result<AnalyzedFile<'a>, AnalyzeError> {
        let source_type =
            SourceType::from_path(&file.path).map_err(|_| AnalyzeError::UnsupportedSource)?;
        let result = Parser::new(allocator, &file.source, source_type)
            .with_options(ParseOptions {
                preserve_parens: false,
                ..ParseOptions::default()
            })
            .parse();

        Ok(AnalyzedFile {
            path: &file.path,
            source: &file.source,
            changes: ChangeMetadata::between(&file.previous, &file.source),
            syntax_errors: result
                .diagnostics
                .iter()
                .map(|diagnostic| format!("{diagnostic:?}"))
                .collect(),
            panicked: result.panicked,
            ast: result.program,
        })
    }
}

#[cfg(test)]
mod tests;
