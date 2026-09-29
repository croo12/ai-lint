use super::*;
use oxc_ast::ast::Statement;

#[test]
fn returns_a_typed_ast_without_evaluating_rules() {
    let file = ChangedFile::new(
        "test.ts",
        "const greeting: string = 'hello'; alert(greeting);",
    );
    let allocator = Allocator::default();
    let analyzed = Analyzer::analyze(&allocator, &file).unwrap();

    assert!(analyzed.is_valid());
    assert_eq!(analyzed.path, Path::new("test.ts"));
    assert_eq!(analyzed.source, file.source);
    assert_eq!(analyzed.ast.body.len(), 2);
    assert!(matches!(
        analyzed.ast.body[0],
        Statement::VariableDeclaration(_)
    ));
    assert_eq!(analyzed.changes.kind, ChangeKind::Unknown);
    assert_eq!(analyzed.changes.previous_bytes, None);
    assert_eq!(analyzed.changes.current_bytes, file.source.len());
    assert_eq!(analyzed.changes.range, None);
}

#[test]
fn parses_each_supported_source_type() {
    for (path, source) in [
        ("file.js", "const value = 1;"),
        ("file.jsx", "const element = <div />;"),
        ("file.ts", "const value: number = 1;"),
        ("file.tsx", "const element = <div />;"),
        ("file.mjs", "export const value = 1;"),
        ("file.cjs", "module.exports = 1;"),
        ("file.mts", "export const value: number = 1;"),
        ("file.cts", "const value: number = 1;"),
    ] {
        let file = ChangedFile::new(path, source);
        let allocator = Allocator::default();
        let analyzed = Analyzer::analyze(&allocator, &file).unwrap();
        assert!(analyzed.is_valid(), "{path}: {:?}", analyzed.syntax_errors);
    }
}

#[test]
fn invalid_syntax_still_returns_change_metadata_and_diagnostics() {
    let file = ChangedFile::new("test.ts", "const greeting: = 'hello';")
        .with_previous_source("const greeting: string = 'hello';");
    let allocator = Allocator::default();
    let analyzed = Analyzer::analyze(&allocator, &file).unwrap();

    assert!(!analyzed.is_valid());
    assert!(!analyzed.syntax_errors.is_empty());
    assert_eq!(analyzed.changes.kind, ChangeKind::Modified);
    assert_eq!(analyzed.changes.range.unwrap().after, 16..16);
}

#[test]
fn reads_a_snapshot_and_reports_input_errors() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = root.join("tests/fixtures/effect_without_setter.tsx");
    let file = ChangedFile::read(&path).unwrap();
    assert_eq!(file.path, path);
    assert_eq!(file.previous, PreviousSource::Unknown);
    let allocator = Allocator::default();
    assert!(Analyzer::analyze(&allocator, &file).unwrap().is_valid());
    assert!(matches!(
        ChangedFile::read(root.join("tests/fixtures/missing.tsx")),
        Err(AnalyzeError::Read(_))
    ));
    let unsupported = ChangedFile::new("file.txt", "const x = 1;");
    assert!(matches!(
        Analyzer::analyze(&allocator, &unsupported),
        Err(AnalyzeError::UnsupportedSource)
    ));
}

#[test]
fn distinguishes_added_unchanged_and_unknown_baselines() {
    for source in ["", "const value = 1;\r\n"] {
        let allocator = Allocator::default();
        let added = ChangedFile::added("test.ts", source);
        let analyzed = Analyzer::analyze(&allocator, &added).unwrap();
        assert_eq!(analyzed.changes.kind, ChangeKind::Added);
        assert_eq!(analyzed.changes.previous_bytes, None);
        assert_eq!(
            analyzed.changes.range,
            Some(ChangedRange {
                before: 0..0,
                after: 0..source.len()
            })
        );

        let unchanged = ChangedFile::new("test.ts", source).with_previous_source(source);
        let analyzed = Analyzer::analyze(&allocator, &unchanged).unwrap();
        assert_eq!(analyzed.changes.kind, ChangeKind::Unchanged);
        assert_eq!(analyzed.changes.previous_bytes, Some(source.len()));
        assert_eq!(analyzed.changes.range, None);
    }
    let existing_empty = ChangedFile::new("test.ts", "const value = 1;").with_previous_source("");
    let allocator = Allocator::default();
    let analyzed = Analyzer::analyze(&allocator, &existing_empty).unwrap();
    assert_eq!(analyzed.changes.kind, ChangeKind::Modified);
    assert_eq!(analyzed.changes.previous_bytes, Some(0));
}

#[test]
fn change_ranges_cover_insertions_deletions_and_unicode_without_splitting_characters() {
    for (previous, current, before, after) in [
        ("const x = 1;", "const x = 12;", "", "2"),
        ("const x = 12;", "const x = 1;", "2", ""),
        ("const x = '가';", "const x = '각';", "가", "각"),
        ("// 😀\nconst x = 1;", "// 😁\nconst x = 1;", "😀", "😁"),
        ("// 한글\r\nconst x = 1;", "// 한글\nconst x = 1;", "\r", ""),
        ("", "alert(1);", "", "alert(1);"),
        ("alert(1);", "", "alert(1);", ""),
        ("aaa", "aaaa", "", "a"),
        ("aaaa", "aaa", "a", ""),
        ("abc", "XYZ", "abc", "XYZ"),
        (
            "let a = 1; let b = 2;",
            "let a = 3; let b = 4;",
            "1; let b = 2",
            "3; let b = 4",
        ),
    ] {
        let file = ChangedFile::new("test.ts", current).with_previous_source(previous);
        let allocator = Allocator::default();
        let analyzed = Analyzer::analyze(&allocator, &file).unwrap();
        assert_eq!(analyzed.changes.kind, ChangeKind::Modified);
        assert_eq!(analyzed.changes.previous_bytes, Some(previous.len()));
        assert_eq!(analyzed.changes.current_bytes, current.len());
        let range = analyzed.changes.range.unwrap();
        assert_eq!(&previous[range.before.clone()], before);
        assert_eq!(&current[range.after.clone()], after);
        assert_eq!(
            format!(
                "{}{}{}",
                &previous[..range.before.start],
                after,
                &previous[range.before.end..]
            ),
            current
        );
    }
}
