use crate::support::run_powershell;
use base64::{engine::general_purpose::STANDARD, Engine};
use std::fs;
use std::path::{Path, PathBuf};

const APP_DIR_NAME: &str = "rust-desk-light";
const AUTOSTART_ITEM_NAME: &str = "rust-desk-light-client";
const WINDOWS_RUN_VALUE: &str = "RustDeskLightClient";
const WINDOWS_RUN_KEY: &str = "HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const WINDOWS_RUN_DISABLED_KEY: &str =
    "HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\RunDisabled";
const MACOS_LAUNCH_AGENT_LABEL: &str = "com.rust-desk-light.client";

pub(super) fn apply_startup_manager_action(action: &str) -> Result<(), String> {
    let paths = AutostartPaths::detect()?;
    match action {
        "enable" => {
            install_current_binary(&paths)?;
            enable_autostart(&paths)
        }
        "disable" => disable_autostart(&paths),
        _ => Err(format!("unsupported client_autostart action: {action}")),
    }
}

struct AutostartPaths {
    current_exe: PathBuf,
    target_exe: PathBuf,
    entry_path: String,
}

impl AutostartPaths {
    fn detect() -> Result<Self, String> {
        let current_exe =
            std::env::current_exe().map_err(|error| format!("current exe unavailable: {error}"))?;
        let file_name = current_exe
            .file_name()
            .filter(|name| !name.to_string_lossy().is_empty())
            .map(|name| name.to_os_string())
            .unwrap_or_else(|| default_exe_name().into());
        let target_dir = stable_client_dir()?;
        let target_exe = target_dir.join(file_name);
        let entry_path = if cfg!(target_os = "windows") {
            format!("{WINDOWS_RUN_KEY}\\{WINDOWS_RUN_VALUE}")
        } else if cfg!(target_os = "macos") {
            home_dir()?
                .join("Library")
                .join("LaunchAgents")
                .join(format!("{MACOS_LAUNCH_AGENT_LABEL}.plist"))
                .display()
                .to_string()
        } else {
            home_dir()?
                .join(".config")
                .join("autostart")
                .join(format!("{AUTOSTART_ITEM_NAME}.desktop"))
                .display()
                .to_string()
        };

        Ok(Self {
            current_exe,
            target_exe,
            entry_path,
        })
    }
}

fn install_current_binary(paths: &AutostartPaths) -> Result<(), String> {
    if same_path(&paths.current_exe, &paths.target_exe) {
        return Ok(());
    }
    if let Some(parent) = paths.target_exe.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "create autostart target directory {} failed: {error}",
                parent.display()
            )
        })?;
    }
    fs::copy(&paths.current_exe, &paths.target_exe).map_err(|error| {
        format!(
            "copy client binary from {} to {} failed: {error}",
            paths.current_exe.display(),
            paths.target_exe.display()
        )
    })?;
    Ok(())
}

fn enable_autostart(paths: &AutostartPaths) -> Result<(), String> {
    if cfg!(target_os = "windows") {
        windows_enable_autostart(&paths.target_exe)
    } else if cfg!(target_os = "macos") {
        macos_enable_autostart(paths)
    } else {
        linux_enable_autostart(paths)
    }
}

fn disable_autostart(paths: &AutostartPaths) -> Result<(), String> {
    if cfg!(target_os = "windows") {
        windows_disable_autostart()
    } else if cfg!(target_os = "macos") {
        rename_autostart_entry_disabled(paths)
    } else {
        rename_autostart_entry_disabled(paths)
    }
}

fn windows_enable_autostart(target_exe: &Path) -> Result<(), String> {
    let command = windows_run_command(target_exe);
    let script = r#"
$ErrorActionPreference = "Stop"
function Decode($value) {
  [System.Text.Encoding]::UTF8.GetString([System.Convert]::FromBase64String($value))
}
$key = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
$disabledKey = "__DISABLED_KEY__"
$name = "RustDeskLightClient"
$value = Decode "__COMMAND_B64__"
if (!(Test-Path $key)) {
  New-Item -Path $key -Force | Out-Null
}
New-ItemProperty -Path $key -Name $name -Value $value -PropertyType String -Force | Out-Null
if (Test-Path $disabledKey) {
  Remove-ItemProperty -Path $disabledKey -Name $name -ErrorAction SilentlyContinue
}
Write-Output "ok"
"#
    .replace("__COMMAND_B64__", &encode_text(&command))
    .replace("__DISABLED_KEY__", WINDOWS_RUN_DISABLED_KEY);
    powershell_result(
        run_powershell(&script, 20),
        "enable Windows login autostart",
    )
}

fn windows_disable_autostart() -> Result<(), String> {
    let script = r#"
$ErrorActionPreference = "Stop"
$key = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
$disabledKey = "__DISABLED_KEY__"
$name = "RustDeskLightClient"
if (Test-Path $key) {
  $property = (Get-ItemProperty -Path $key).PSObject.Properties | Where-Object { $_.Name -eq $name } | Select-Object -First 1
  if ($null -ne $property) {
    if (!(Test-Path $disabledKey)) {
      New-Item -Path $disabledKey -Force | Out-Null
    }
    New-ItemProperty -Path $disabledKey -Name $name -Value $property.Value -PropertyType String -Force | Out-Null
    Remove-ItemProperty -Path $key -Name $name -ErrorAction Stop
  }
}
Write-Output "ok"
"#
    .replace("__DISABLED_KEY__", WINDOWS_RUN_DISABLED_KEY);
    powershell_result(
        run_powershell(&script, 20),
        "disable Windows login autostart",
    )
}

fn linux_enable_autostart(paths: &AutostartPaths) -> Result<(), String> {
    let entry_path = Path::new(&paths.entry_path);
    let disabled_path = disabled_entry_path(entry_path);
    if let Some(parent) = entry_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "create autostart directory {} failed: {error}",
                parent.display()
            )
        })?;
    }
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=rust-desk-light Client\nExec={}\nHidden=false\nX-GNOME-Autostart-enabled=true\n",
        desktop_exec_value(&paths.target_exe)
    );
    fs::write(entry_path, entry).map_err(|error| {
        format!(
            "write autostart entry {} failed: {error}",
            entry_path.display()
        )
    })?;
    remove_file_if_exists(&disabled_path, "remove disabled autostart entry")
}

fn macos_enable_autostart(paths: &AutostartPaths) -> Result<(), String> {
    let entry_path = Path::new(&paths.entry_path);
    let disabled_path = disabled_entry_path(entry_path);
    if let Some(parent) = entry_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "create LaunchAgents directory {} failed: {error}",
                parent.display()
            )
        })?;
    }
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{}</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
</dict>
</plist>
"#,
        xml_escape(MACOS_LAUNCH_AGENT_LABEL),
        xml_escape(&path_text(&paths.target_exe))
    );
    fs::write(entry_path, plist).map_err(|error| {
        format!(
            "write launch agent {} failed: {error}",
            entry_path.display()
        )
    })?;
    remove_file_if_exists(&disabled_path, "remove disabled launch agent")
}

fn rename_autostart_entry_disabled(paths: &AutostartPaths) -> Result<(), String> {
    let entry_path = Path::new(&paths.entry_path);
    let disabled_path = disabled_entry_path(entry_path);
    if !entry_path.exists() {
        return Ok(());
    }
    remove_file_if_exists(&disabled_path, "replace disabled autostart entry")?;
    fs::rename(entry_path, &disabled_path).map_err(|error| {
        format!(
            "disable autostart entry {} failed: {error}",
            entry_path.display()
        )
    })
}

fn disabled_entry_path(entry_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.disabled", entry_path.display()))
}

fn remove_file_if_exists(path: &Path, context: &str) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("{context} {} failed: {error}", path.display())),
    }
}

fn powershell_result(output: String, context: &str) -> Result<(), String> {
    let text = output.trim();
    let lower = text.to_ascii_lowercase();
    if lower.starts_with("powershell exited with error")
        || lower.starts_with("powershell failed:")
        || lower.contains(" timed out")
    {
        Err(format!("{context} failed: {text}"))
    } else {
        Ok(())
    }
}

fn stable_client_dir() -> Result<PathBuf, String> {
    if cfg!(target_os = "windows") {
        std::env::var_os("LOCALAPPDATA")
            .or_else(|| std::env::var_os("APPDATA"))
            .map(PathBuf::from)
            .or_else(|| {
                home_dir()
                    .ok()
                    .map(|home| home.join("AppData").join("Local"))
            })
            .map(|path| path.join(APP_DIR_NAME))
            .ok_or_else(|| "LOCALAPPDATA is unavailable".to_string())
    } else if cfg!(target_os = "macos") {
        Ok(home_dir()?
            .join("Library")
            .join("Application Support")
            .join(APP_DIR_NAME))
    } else {
        let data_home = match std::env::var_os("XDG_DATA_HOME")
            .filter(|value| !value.to_string_lossy().is_empty())
        {
            Some(path) => PathBuf::from(path),
            None => home_dir()?.join(".local").join("share"),
        };
        Ok(data_home.join(APP_DIR_NAME))
    }
}

fn home_dir() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| "home directory is unavailable".to_string())
}

fn default_exe_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "rdl-client-gui.exe"
    } else {
        "rdl-client-gui"
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn windows_run_command(path: &Path) -> String {
    quote_path(path)
}

fn desktop_exec_value(path: &Path) -> String {
    quote_path(path)
}

fn quote_path(path: &Path) -> String {
    let text = path_text(path).replace('"', "\\\"");
    format!("\"{text}\"")
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn path_text(path: &Path) -> String {
    path.display().to_string()
}

fn encode_text(value: &str) -> String {
    STANDARD.encode(value)
}

#[cfg(test)]
mod tests {
    use super::{disabled_entry_path, quote_path};
    use std::path::Path;

    #[test]
    fn quote_path_wraps_executable_paths() {
        assert_eq!(
            quote_path(Path::new("C:\\Program Files\\rdl.exe")),
            "\"C:\\Program Files\\rdl.exe\""
        );
    }

    #[test]
    fn disabled_entry_path_keeps_original_extension_visible() {
        assert_eq!(
            disabled_entry_path(Path::new(
                "/home/me/.config/autostart/rust-desk-light-client.desktop"
            )),
            Path::new("/home/me/.config/autostart/rust-desk-light-client.desktop.disabled")
        );
    }
}
