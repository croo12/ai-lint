use std::{collections::HashMap, env, fs, path::PathBuf};

fn main() {
    let repository = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let env_file = repository.join(".env");
    println!("cargo:rerun-if-changed={}", env_file.display());
    let values = match dotenvy::from_path_iter(&env_file) {
        Ok(entries) => entries
            .collect::<Result<HashMap<_, _>, _>>()
            .unwrap_or_else(|_| panic!("could not parse build-time model env file")),
        Err(dotenvy::Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            HashMap::new()
        }
        Err(_) => panic!("could not read build-time model env file"),
    };
    let mut generated = String::from("const BUILT_MODEL_SETTINGS: &[(&str, &str)] = &[\n");
    for key in [
        "AI_LINT_MODEL_BASE_URL",
        "AI_LINT_MODEL_NAME",
        "AI_LINT_MODEL_API_KEY",
        "AI_LINT_MODEL_TIMEOUT_SECS",
        "AI_LINT_MODEL_RESPONSE_FORMAT",
        "AI_LINT_MODEL_JSON_MODE",
    ] {
        if let Some(value) = values.get(key) {
            generated.push_str(&format!("({key:?}, {value:?}),\n"));
        }
    }
    generated.push_str("];\n");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("model_settings.rs");
    fs::write(output, generated).expect("could not write build-time model settings");
}
