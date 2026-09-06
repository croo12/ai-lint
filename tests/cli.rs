use std::{path::Path, process::Command};

fn cli() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ai-lint"));
    for name in [
        "AI_LINT_MODEL_BASE_URL",
        "AI_LINT_MODEL_NAME",
        "AI_LINT_MODEL_API_KEY",
        "AI_LINT_MODEL_TIMEOUT_SECS",
        "AI_LINT_MODEL_JSON_MODE",
    ] {
        command.env_remove(name);
    }
    command
        .arg("check")
        .arg("--env-file")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join(".env.example"));
    command
}

#[test]
fn check_prints_rule_message_and_sets_exit_code() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let invalid = cli()
        .arg(fixtures.join("effect_with_setter.tsx"))
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(1));
    let stderr = String::from_utf8(invalid.stderr).unwrap();
    assert!(stderr.contains("[no-set-state-in-effect] useEffect에 setState를 넣어서는 안됩니다"));
    assert!(stderr.contains("Found 1 finding(s)"));

    let valid = cli()
        .arg(fixtures.join("effect_without_setter.tsx"))
        .output()
        .unwrap();
    assert_eq!(valid.status.code(), Some(0));
    assert!(valid.stderr.is_empty());
}

#[test]
fn config_errors_exit_two_and_do_not_echo_values() {
    let output = cli()
        .env("AI_LINT_MODEL_BASE_URL", "invalid-secret-url")
        .env("AI_LINT_MODEL_NAME", "model")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/effect_without_setter.tsx"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("AI_LINT_MODEL_BASE_URL"));
    assert!(!stderr.contains("invalid-secret-url"));
}

#[test]
fn env_file_loads_and_environment_overrides_it_without_model_calls_for_static_rules() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for override_timeout in [None, Some("bad")] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ai-lint"));
        for key in [
            "AI_LINT_MODEL_BASE_URL",
            "AI_LINT_MODEL_NAME",
            "AI_LINT_MODEL_API_KEY",
            "AI_LINT_MODEL_TIMEOUT_SECS",
            "AI_LINT_MODEL_JSON_MODE",
        ] {
            command.env_remove(key);
        }
        if let Some(value) = override_timeout {
            command.env("AI_LINT_MODEL_TIMEOUT_SECS", value);
        }
        let output = command
            .arg("check")
            .arg("--env-file")
            .arg(root.join("tests/fixtures/model-config.env"))
            .arg(root.join("tests/fixtures/effect_without_setter.tsx"))
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(if override_timeout.is_some() { 2 } else { 0 })
        );
    }
}

#[test]
fn yaml_rules_run_and_duplicate_ids_are_rejected() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let rule = root.join("rules/no-set-state-in-effect.yaml");
    let file = root.join("tests/fixtures/effect_with_setter.tsx");
    let output = cli().arg("--rules").arg(&rule).arg(&file).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Found 1 finding(s)")
    );
    let output = cli()
        .arg("--rules")
        .arg(&rule)
        .arg("--rules")
        .arg(&rule)
        .arg(&file)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("duplicate rule id")
    );
}

#[test]
fn yaml_load_and_model_errors_exit_two() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for rule in [
        "rules/missing.yaml",
        "rules/examples/contextual-effect.yaml",
        "tests/fixtures/effect_with_setter.tsx",
    ] {
        let output = cli()
            .arg("--rules")
            .arg(root.join(rule))
            .arg(root.join("tests/fixtures/effect_with_setter.tsx"))
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{rule}");
    }
}
