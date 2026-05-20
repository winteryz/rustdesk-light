use super::shared::{clean_value, payload_field};
use base64::{engine::general_purpose::STANDARD, Engine};
#[cfg(unix)]
use std::io::Write;
use std::process::{Command, Stdio};

pub(super) fn handle(payload: &str) -> String {
    let request = match TaskRequest::parse(payload) {
        Ok(request) => request,
        Err(message) => {
            return format!(
                "create_task\nstatus=failed\nmessage={}",
                clean_value(&message)
            )
        }
    };

    match create_task(&request) {
        Ok(detail) => format!(
            "create_task\nstatus=success\ntask_name={}\ntrigger={}\n{}",
            clean_value(&request.name),
            clean_value(request.trigger.as_str()),
            detail
        ),
        Err(message) => format!(
            "create_task\nstatus=failed\ntask_name={}\ntrigger={}\nmessage={}",
            clean_value(&request.name),
            clean_value(request.trigger.as_str()),
            clean_value(&message)
        ),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TaskRequest {
    name: String,
    command: String,
    trigger: TaskTrigger,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TaskTrigger {
    Startup,
    Daily { time: String },
}

impl TaskTrigger {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::Daily { .. } => "daily",
        }
    }
}

impl TaskRequest {
    fn parse(payload: &str) -> Result<Self, String> {
        let name = payload_field(payload, "name")
            .map(|value| sanitize_single_line(&value))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "task name is required".to_string())?;
        let command = payload_field(payload, "command_b64")
            .and_then(|value| STANDARD.decode(value).ok())
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .or_else(|| payload_field(payload, "command"))
            .map(|value| sanitize_single_line(&value))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "command is required".to_string())?;
        let trigger = match payload_field(payload, "trigger").as_deref() {
            Some("daily") => {
                let time = payload_field(payload, "time")
                    .map(|value| sanitize_single_line(&value))
                    .filter(|value| valid_hhmm(value))
                    .ok_or_else(|| "daily task time must be HH:MM".to_string())?;
                TaskTrigger::Daily { time }
            }
            Some("startup") | None | Some("") => TaskTrigger::Startup,
            Some(other) => return Err(format!("unsupported trigger: {other}")),
        };
        Ok(Self {
            name,
            command,
            trigger,
        })
    }
}

fn create_task(request: &TaskRequest) -> Result<String, String> {
    if cfg!(target_os = "windows") {
        create_windows_task(request)
    } else if cfg!(unix) {
        create_cron_task(request)
    } else {
        Err("task creation is not supported on this platform".to_string())
    }
}

#[cfg(target_os = "windows")]
fn create_windows_task(request: &TaskRequest) -> Result<String, String> {
    let task_name = windows_task_name(&request.name);
    let mut args = vec![
        "/Create".to_string(),
        "/F".to_string(),
        "/TN".to_string(),
        task_name.clone(),
        "/TR".to_string(),
        request.command.clone(),
    ];
    match &request.trigger {
        TaskTrigger::Startup => {
            args.push("/SC".to_string());
            args.push("ONLOGON".to_string());
        }
        TaskTrigger::Daily { time } => {
            args.push("/SC".to_string());
            args.push("DAILY".to_string());
            args.push("/ST".to_string());
            args.push(time.clone());
        }
    }

    let output = Command::new("schtasks")
        .args(&args)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("schtasks failed to start: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "schtasks failed: {}",
            clean_value(&decode_output(&output.stderr))
        ));
    }
    Ok(format!(
        "schedule={}\nmessage=task created\nstdout:\n{}",
        clean_value(&task_name),
        clean_value(&decode_output(&output.stdout))
    ))
}

#[cfg(not(target_os = "windows"))]
fn create_windows_task(_request: &TaskRequest) -> Result<String, String> {
    Err("Windows scheduled tasks are not supported on this platform".to_string())
}

#[cfg(unix)]
fn create_cron_task(request: &TaskRequest) -> Result<String, String> {
    let marker = cron_marker(&request.name);
    let line = cron_line(request, &marker);
    let current = current_crontab()?;
    let mut lines = current
        .lines()
        .filter(|line| !line.contains(&marker))
        .map(str::to_string)
        .collect::<Vec<_>>();
    lines.push(line.clone());
    install_crontab(&lines.join("\n"))?;
    Ok(format!(
        "schedule={}\nmessage=cron entry installed\nstdout:\n{}",
        clean_value(&line),
        clean_value("ok")
    ))
}

#[cfg(not(unix))]
fn create_cron_task(_request: &TaskRequest) -> Result<String, String> {
    Err("cron tasks are not supported on this platform".to_string())
}

#[cfg(unix)]
fn current_crontab() -> Result<String, String> {
    let output = Command::new("crontab")
        .arg("-l")
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("crontab -l failed to start: {error}"))?;
    if output.status.success() {
        return Ok(decode_output(&output.stdout));
    }
    let stderr = decode_output(&output.stderr);
    if stderr.to_ascii_lowercase().contains("no crontab") {
        Ok(String::new())
    } else {
        Err(format!("crontab -l failed: {}", clean_value(&stderr)))
    }
}

#[cfg(unix)]
fn install_crontab(content: &str) -> Result<(), String> {
    let mut child = Command::new("crontab")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("crontab install failed to start: {error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(content.as_bytes())
            .and_then(|_| stdin.write_all(b"\n"))
            .map_err(|error| format!("write crontab failed: {error}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("wait for crontab failed: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "crontab install failed: {}",
            clean_value(&decode_output(&output.stderr))
        ))
    }
}

#[cfg(unix)]
fn cron_line(request: &TaskRequest, marker: &str) -> String {
    let command = cron_command(&request.command);
    match &request.trigger {
        TaskTrigger::Startup => format!("@reboot {command} {marker}"),
        TaskTrigger::Daily { time } => {
            let (hour, minute) = time.split_once(':').unwrap_or(("00", "00"));
            format!("{minute} {hour} * * * {command} {marker}")
        }
    }
}

#[cfg(unix)]
fn cron_marker(name: &str) -> String {
    format!("# rdl-task:{}", name.replace(['#', '\r', '\n'], " ").trim())
}

#[cfg(unix)]
fn cron_command(command: &str) -> String {
    sanitize_single_line(command).replace('%', r"\%")
}

#[cfg(target_os = "windows")]
fn windows_task_name(name: &str) -> String {
    format!(
        "RustDeskLight-{}",
        name.replace(['\\', '/', ':'], "-").trim()
    )
}

fn sanitize_single_line(value: &str) -> String {
    value.replace(['\t', '\r', '\n'], " ").trim().to_string()
}

fn valid_hhmm(value: &str) -> bool {
    let Some((hour, minute)) = value.split_once(':') else {
        return false;
    };
    hour.len() == 2
        && minute.len() == 2
        && hour.parse::<u8>().map(|value| value <= 23).unwrap_or(false)
        && minute
            .parse::<u8>()
            .map(|value| value <= 59)
            .unwrap_or(false)
}

fn decode_output(bytes: &[u8]) -> String {
    crate::text_decode::command_output(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_daily_task_request() {
        let request =
            TaskRequest::parse("name=Nightly\ntrigger=daily\ntime=23:15\ncommand_b64=ZWNobyBoaQ==")
                .unwrap();
        assert_eq!(request.name, "Nightly");
        assert_eq!(request.command, "echo hi");
        assert_eq!(
            request.trigger,
            TaskTrigger::Daily {
                time: "23:15".to_string()
            }
        );
    }

    #[test]
    fn rejects_invalid_daily_time() {
        let error =
            TaskRequest::parse("name=Nightly\ntrigger=daily\ntime=25:99\ncommand_b64=ZWNobyBoaQ==")
                .unwrap_err();
        assert!(error.contains("HH:MM"));
    }
}
