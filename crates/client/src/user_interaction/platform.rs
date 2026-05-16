#[cfg(target_os = "windows")]
pub(super) fn powershell_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(target_os = "macos")]
pub(super) fn applescript_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\r', "")
        .replace('\n', "\\n")
}

#[cfg(any(target_os = "macos", all(unix, not(target_os = "macos"))))]
pub(super) fn command_status(program: &str, args: &[&str]) -> Result<(), String> {
    use std::process::{Command, Stdio};

    let status = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("{program} failed: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} exited with error"))
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(super) fn run_first_success(commands: &[(&str, Vec<&str>)]) -> Result<(), String> {
    let mut errors = Vec::new();
    for (program, args) in commands {
        match command_status(program, args) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error),
        }
    }
    Err(errors
        .last()
        .cloned()
        .unwrap_or_else(|| "no supported GUI command found".to_string()))
}
