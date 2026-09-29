use std::{collections::BTreeSet, process::Command};

/// Covers normal, build, and dev dependencies, including renamed dependencies.
/// Integration tests belong in the CLI app so they cannot open back doors
/// between otherwise independent crates.
#[test]
fn workspace_dependencies_preserve_architecture_boundaries() {
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--locked",
            "--offline",
        ])
        .current_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .output()
        .expect("cargo metadata must run");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let packages = metadata["packages"].as_array().unwrap();
    let boundaries: &[(&str, &[&str])] = &[
        (
            "ai-lint",
            &["ai-lint-analyzer", "ai-lint-rule-engine", "ai-lint-rules"],
        ),
        ("ai-lint-analyzer", &[]),
        ("ai-lint-rule-contract", &[]),
        ("ai-lint-rule-engine", &["ai-lint-rule-contract"]),
        ("ai-lint-rules", &["ai-lint-rule-contract"]),
    ];
    let expected: BTreeSet<_> = boundaries.iter().map(|(name, _)| *name).collect();
    let actual: BTreeSet<_> = packages
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        actual, expected,
        "register new workspace crates with their allowed dependencies"
    );

    for (name, allowed) in boundaries {
        let package = packages.iter().find(|p| p["name"] == *name).unwrap();
        for dependency in package["dependencies"].as_array().unwrap() {
            let dependency_name = dependency["name"].as_str().unwrap();
            if actual.contains(dependency_name) {
                assert!(
                    allowed.contains(&dependency_name),
                    "forbidden workspace dependency: {name} -> {dependency_name} (kind: {})",
                    dependency["kind"],
                );
            }
        }
    }

    let member_ids = |key: &str| -> BTreeSet<_> {
        metadata[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_str().unwrap())
            .collect()
    };
    assert_eq!(
        member_ids("workspace_default_members"),
        member_ids("workspace_members"),
        "plain cargo test must include every workspace crate",
    );
}
