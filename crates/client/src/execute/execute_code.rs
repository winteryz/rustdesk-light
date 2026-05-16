use super::shared::{clean_value, command_available, now_millis, payload_field, run_process};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::fs;

pub(super) fn handle(payload: &str) -> String {
    match payload_field(payload, "action").as_deref() {
        Some("languages") => execute_code_languages(),
        _ => run_code(payload),
    }
}

fn execute_code_languages() -> String {
    let mut rows = vec!["Language\tCommand\tStatus".to_string()];
    for runtime in language_runtimes() {
        if command_available(runtime.command) {
            rows.push(format!("{}\t{}\tavailable", runtime.id, runtime.command));
        }
    }
    if rows.len() == 1 {
        rows.push("none\t-\tNo supported language found".to_string());
    }
    format!("execute_code_languages:\n{}", rows.join("\n"))
}

fn run_code(payload: &str) -> String {
    let language = payload_field(payload, "language").unwrap_or_default();
    let Some(runtime) = language_runtimes()
        .into_iter()
        .find(|runtime| runtime.id == language)
    else {
        return format!(
            "execute_code\nstatus=failed\nlanguage={}\nmessage=unsupported language",
            clean_value(&language)
        );
    };
    if !command_available(runtime.command) {
        return format!(
            "execute_code\nstatus=failed\nlanguage={}\nmessage=language runtime is not available",
            clean_value(runtime.id)
        );
    }
    let Some(code) = payload_field(payload, "code_b64")
        .and_then(|value| STANDARD.decode(value).ok())
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .filter(|value| !value.trim().is_empty())
    else {
        return format!(
            "execute_code\nstatus=failed\nlanguage={}\nmessage=code is required",
            clean_value(runtime.id)
        );
    };

    let path = std::env::temp_dir().join(format!(
        "rdl-execute-{}-{}.{}",
        std::process::id(),
        now_millis(),
        runtime.extension
    ));
    if let Err(error) = fs::write(&path, code) {
        return format!(
            "execute_code\nstatus=failed\nlanguage={}\nmessage=write temp file failed: {}",
            clean_value(runtime.id),
            clean_value(&error.to_string())
        );
    }
    let path = path.display().to_string();
    let args = runtime_args(&runtime, &path);
    let output = run_process(runtime.command, &args, None);
    let _ = fs::remove_file(&path);
    format!(
        "execute_code\nlanguage={}\ncommand={}\n{}",
        clean_value(runtime.id),
        clean_value(runtime.command),
        output
    )
}

#[derive(Clone, Copy)]
struct LanguageRuntime {
    id: &'static str,
    command: &'static str,
    extension: &'static str,
}

fn language_runtimes() -> Vec<LanguageRuntime> {
    let mut runtimes = vec![
        LanguageRuntime {
            id: "python3",
            command: "python3",
            extension: "py",
        },
        LanguageRuntime {
            id: "python",
            command: "python",
            extension: "py",
        },
        LanguageRuntime {
            id: "node",
            command: "node",
            extension: "js",
        },
    ];
    if cfg!(target_os = "windows") {
        runtimes.push(LanguageRuntime {
            id: "powershell",
            command: "powershell",
            extension: "ps1",
        });
    } else {
        runtimes.push(LanguageRuntime {
            id: "bash",
            command: "bash",
            extension: "sh",
        });
        runtimes.push(LanguageRuntime {
            id: "sh",
            command: "sh",
            extension: "sh",
        });
    }
    runtimes
}

fn runtime_args(runtime: &LanguageRuntime, path: &str) -> Vec<String> {
    if cfg!(target_os = "windows") && runtime.id == "powershell" {
        return vec![
            "-NoProfile".to_string(),
            "-ExecutionPolicy".to_string(),
            "Bypass".to_string(),
            "-File".to_string(),
            path.to_string(),
        ];
    }
    vec![path.to_string()]
}
