use crate::windowing;
use eframe::egui;
use rdl_protocol::CommandKind;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

const COLOR_BG: egui::Color32 = egui::Color32::from_rgb(246, 248, 251);
const COLOR_BORDER: egui::Color32 = egui::Color32::from_rgb(222, 228, 236);
const COLOR_PANEL: egui::Color32 = egui::Color32::from_rgb(255, 255, 255);
const COLOR_TEXT: egui::Color32 = egui::Color32::from_rgb(24, 33, 47);
const COLOR_MUTED: egui::Color32 = egui::Color32::from_rgb(96, 108, 124);
const COLOR_BAD: egui::Color32 = egui::Color32::from_rgb(190, 58, 58);
const TOOLBAR_CONTROL_HEIGHT: f32 = 28.0;

pub(crate) struct SessionCommandWindow {
    pub(crate) client_id: String,
    hostname: String,
    username: String,
    command: CommandKind,
    delay_seconds: Arc<Mutex<String>>,
    update_path: Arc<Mutex<String>>,
    client_config_ip: Arc<Mutex<String>>,
    client_config_port: Arc<Mutex<String>>,
    remove_binary: Arc<AtomicBool>,
    client_config_reconnect: Arc<AtomicBool>,
    confirmed: Arc<AtomicBool>,
    open: bool,
    close_requested: Arc<AtomicBool>,
    send_requested: Arc<AtomicBool>,
}

pub(crate) struct OutboundSessionCommand {
    pub(crate) client_id: String,
    pub(crate) command: CommandKind,
    pub(crate) payload: String,
}

pub(crate) fn open_window(
    windows: &mut Vec<SessionCommandWindow>,
    client_id: &str,
    hostname: String,
    username: String,
    command: CommandKind,
    default_ip: &str,
    default_port: u16,
) {
    if let Some(window) = windows
        .iter_mut()
        .find(|window| window.client_id == client_id && window.command == command)
    {
        window.open = true;
        window.hostname = hostname;
        window.username = username;
        window.confirmed.store(false, Ordering::Relaxed);
        if command == CommandKind::ClientConfig {
            if let Ok(mut value) = window.client_config_ip.lock() {
                *value = default_ip.to_string();
            }
            if let Ok(mut value) = window.client_config_port.lock() {
                *value = default_port.to_string();
            }
            window
                .client_config_reconnect
                .store(true, Ordering::Relaxed);
        }
        return;
    }

    windows.push(SessionCommandWindow {
        client_id: client_id.to_string(),
        hostname,
        username,
        command,
        delay_seconds: Arc::new(Mutex::new(default_delay_seconds().to_string())),
        update_path: Arc::new(Mutex::new(String::new())),
        client_config_ip: Arc::new(Mutex::new(default_ip.to_string())),
        client_config_port: Arc::new(Mutex::new(default_port.to_string())),
        remove_binary: Arc::new(AtomicBool::new(false)),
        client_config_reconnect: Arc::new(AtomicBool::new(true)),
        confirmed: Arc::new(AtomicBool::new(false)),
        open: true,
        close_requested: Arc::new(AtomicBool::new(false)),
        send_requested: Arc::new(AtomicBool::new(false)),
    });
}

pub(crate) fn render_windows(
    ctx: &egui::Context,
    windows: &mut Vec<SessionCommandWindow>,
) -> Vec<OutboundSessionCommand> {
    let mut outbound = Vec::new();
    for window in windows.iter_mut() {
        if window.close_requested.load(Ordering::Relaxed) {
            window.open = false;
        }
        if !window.open {
            continue;
        }

        let client_id = window.client_id.clone();
        let title = format!(
            "{} - {}",
            command_title(&window.command),
            identity_title(&window.hostname, &window.username)
        );
        let viewport_id = egui::ViewportId::from_hash_of((
            "admin_session_command",
            &client_id,
            window.command.as_str(),
        ));
        let builder = windowing::child_viewport_builder(title, [520.0, 360.0], [420.0, 280.0]);

        let command = window.command.clone();
        let delay_seconds = window.delay_seconds.clone();
        let update_path = window.update_path.clone();
        let client_config_ip = window.client_config_ip.clone();
        let client_config_port = window.client_config_port.clone();
        let remove_binary = window.remove_binary.clone();
        let client_config_reconnect = window.client_config_reconnect.clone();
        let confirmed = window.confirmed.clone();
        let close_requested = window.close_requested.clone();
        let send_requested = window.send_requested.clone();

        ctx.show_viewport_immediate(viewport_id, builder, move |ui, _class| {
            if ui.ctx().input(|input| input.viewport().close_requested()) {
                close_requested.store(true, Ordering::Relaxed);
            }
            egui::CentralPanel::default()
                .frame(egui::Frame::default().fill(COLOR_BG).inner_margin(12.0))
                .show_inside(ui, |ui| {
                    windowing::render_child_window_controls(ui);
                    render_form(
                        ui,
                        &command,
                        &delay_seconds,
                        &update_path,
                        &client_config_ip,
                        &client_config_port,
                        &remove_binary,
                        &client_config_reconnect,
                        &confirmed,
                        &send_requested,
                    );
                });
        });

        if window.send_requested.swap(false, Ordering::Relaxed) {
            let delay_seconds = window
                .delay_seconds
                .lock()
                .map(|value| value.clone())
                .unwrap_or_else(|_| default_delay_seconds().to_string());
            let update_path = window
                .update_path
                .lock()
                .map(|value| value.clone())
                .unwrap_or_default();
            let client_config_ip = window
                .client_config_ip
                .lock()
                .map(|value| value.clone())
                .unwrap_or_default();
            let client_config_port = window
                .client_config_port
                .lock()
                .map(|value| value.clone())
                .unwrap_or_default();
            outbound.push(OutboundSessionCommand {
                client_id: client_id.clone(),
                command: window.command.clone(),
                payload: payload_for(
                    &window.command,
                    &delay_seconds,
                    &update_path,
                    &client_config_ip,
                    &client_config_port,
                    window.remove_binary.load(Ordering::Relaxed),
                    window.client_config_reconnect.load(Ordering::Relaxed),
                ),
            });
            window.open = false;
        }
    }

    windows.retain(|window| window.open);
    outbound
}

fn render_form(
    ui: &mut egui::Ui,
    command: &CommandKind,
    delay_seconds: &Arc<Mutex<String>>,
    update_path: &Arc<Mutex<String>>,
    client_config_ip: &Arc<Mutex<String>>,
    client_config_port: &Arc<Mutex<String>>,
    remove_binary: &Arc<AtomicBool>,
    client_config_reconnect: &Arc<AtomicBool>,
    confirmed: &Arc<AtomicBool>,
    send_requested: &Arc<AtomicBool>,
) {
    egui::Frame::default()
        .fill(COLOR_PANEL)
        .stroke(egui::Stroke::new(1.0, COLOR_BORDER))
        .corner_radius(8.0)
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(risk_text(command))
                        .size(13.0)
                        .color(COLOR_BAD)
                        .strong(),
                );
                ui.add_space(10.0);
                render_command_fields(
                    ui,
                    command,
                    delay_seconds,
                    update_path,
                    client_config_ip,
                    client_config_port,
                    remove_binary,
                    client_config_reconnect,
                );
                ui.add_space(10.0);
                render_confirm(ui, confirmed, send_requested);
            });
        });
}

fn render_command_fields(
    ui: &mut egui::Ui,
    command: &CommandKind,
    delay_seconds: &Arc<Mutex<String>>,
    update_path: &Arc<Mutex<String>>,
    client_config_ip: &Arc<Mutex<String>>,
    client_config_port: &Arc<Mutex<String>>,
    remove_binary: &Arc<AtomicBool>,
    client_config_reconnect: &Arc<AtomicBool>,
) {
    match command {
        CommandKind::UpdateClient => {
            render_update_path(ui, update_path);
        }
        CommandKind::ClientConfig => {
            render_client_config(
                ui,
                client_config_ip,
                client_config_port,
                client_config_reconnect,
            );
        }
        CommandKind::Shutdown | CommandKind::Reboot => {
            render_delay(ui, delay_seconds);
        }
        CommandKind::UninstallClient => {
            render_remove_binary(ui, remove_binary);
        }
        CommandKind::KillClientProcess | CommandKind::DeleteClient => {
            ui.label(
                egui::RichText::new("The client will disconnect after acknowledging the command.")
                    .size(12.0)
                    .color(COLOR_MUTED),
            );
        }
        _ => {}
    }
}

fn render_client_config(
    ui: &mut egui::Ui,
    client_config_ip: &Arc<Mutex<String>>,
    client_config_port: &Arc<Mutex<String>>,
    client_config_reconnect: &Arc<AtomicBool>,
) {
    let mut ip = client_config_ip
        .lock()
        .map(|value| value.clone())
        .unwrap_or_default();
    ui.label(
        egui::RichText::new("Server IP")
            .size(12.0)
            .color(COLOR_MUTED),
    );
    let response = ui.add_sized(
        [ui.available_width(), TOOLBAR_CONTROL_HEIGHT],
        egui::TextEdit::singleline(&mut ip)
            .hint_text("127.0.0.1")
            .vertical_align(egui::Align::Center),
    );
    if response.changed() {
        let cleaned = sanitize_single_line(&ip);
        if let Ok(mut value) = client_config_ip.lock() {
            *value = cleaned;
        }
    }

    ui.add_space(8.0);
    let mut port = client_config_port
        .lock()
        .map(|value| value.clone())
        .unwrap_or_default();
    ui.label(
        egui::RichText::new("Server Port")
            .size(12.0)
            .color(COLOR_MUTED),
    );
    let response = ui.add_sized(
        [120.0, TOOLBAR_CONTROL_HEIGHT],
        egui::TextEdit::singleline(&mut port)
            .hint_text("5169")
            .vertical_align(egui::Align::Center),
    );
    if response.changed() {
        let cleaned = port
            .chars()
            .filter(|ch| ch.is_ascii_digit())
            .take(5)
            .collect::<String>();
        if let Ok(mut value) = client_config_port.lock() {
            *value = cleaned;
        }
    }

    ui.add_space(8.0);
    let mut reconnect = client_config_reconnect.load(Ordering::Relaxed);
    if ui
        .checkbox(&mut reconnect, "Reconnect after apply")
        .changed()
    {
        client_config_reconnect.store(reconnect, Ordering::Relaxed);
    }
}

fn render_update_path(ui: &mut egui::Ui, update_path: &Arc<Mutex<String>>) {
    let mut value = update_path
        .lock()
        .map(|value| value.clone())
        .unwrap_or_default();
    ui.label(
        egui::RichText::new("Replacement Binary Path")
            .size(12.0)
            .color(COLOR_MUTED),
    );
    let response = ui.add_sized(
        [ui.available_width(), TOOLBAR_CONTROL_HEIGHT],
        egui::TextEdit::singleline(&mut value)
            .hint_text("Optional path on the client")
            .vertical_align(egui::Align::Center),
    );
    if response.changed() {
        if let Ok(mut update_path) = update_path.lock() {
            *update_path = value;
        }
    }
}

fn render_delay(ui: &mut egui::Ui, delay_seconds: &Arc<Mutex<String>>) {
    let mut value = delay_seconds
        .lock()
        .map(|value| value.clone())
        .unwrap_or_else(|_| default_delay_seconds().to_string());
    ui.label(
        egui::RichText::new("Delay Seconds")
            .size(12.0)
            .color(COLOR_MUTED),
    );
    let response = ui.add_sized(
        [120.0, TOOLBAR_CONTROL_HEIGHT],
        egui::TextEdit::singleline(&mut value)
            .hint_text(default_delay_seconds().to_string())
            .vertical_align(egui::Align::Center),
    );
    if response.changed() {
        let cleaned = value
            .chars()
            .filter(|ch| ch.is_ascii_digit())
            .take(5)
            .collect::<String>();
        if let Ok(mut delay_seconds) = delay_seconds.lock() {
            *delay_seconds = cleaned;
        }
    }
}

fn render_remove_binary(ui: &mut egui::Ui, remove_binary: &Arc<AtomicBool>) {
    let mut value = remove_binary.load(Ordering::Relaxed);
    if ui
        .checkbox(&mut value, "Remove client binary after exit")
        .changed()
    {
        remove_binary.store(value, Ordering::Relaxed);
    }
}

fn render_confirm(
    ui: &mut egui::Ui,
    confirmed: &Arc<AtomicBool>,
    send_requested: &Arc<AtomicBool>,
) {
    let mut value = confirmed.load(Ordering::Relaxed);
    if ui.checkbox(&mut value, "Confirm").changed() {
        confirmed.store(value, Ordering::Relaxed);
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().interact_size.y = TOOLBAR_CONTROL_HEIGHT;
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add_enabled(value, egui::Button::new("Send")).clicked() {
                send_requested.store(true, Ordering::Relaxed);
                ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
            }
            if !value {
                ui.label(
                    egui::RichText::new("Confirmation required")
                        .size(12.0)
                        .color(COLOR_TEXT),
                );
            }
        });
    });
}

fn payload_for(
    command: &CommandKind,
    delay_seconds: &str,
    update_path: &str,
    client_config_ip: &str,
    client_config_port: &str,
    remove_binary: bool,
    client_config_reconnect: bool,
) -> String {
    let mut lines = vec!["confirm=true".to_string()];
    match command {
        CommandKind::UpdateClient => {
            let update_path = update_path.trim();
            if !update_path.is_empty() {
                lines.push(format!("update_path={}", sanitize_single_line(update_path)));
            }
        }
        CommandKind::ClientConfig => {
            let ip = client_config_ip.trim();
            if !ip.is_empty() {
                lines.push(format!("ip={}", sanitize_single_line(ip)));
            }
            let port = client_config_port.trim().parse::<u16>().unwrap_or(5169);
            lines.push(format!("port={port}"));
            lines.push(format!("reconnect={client_config_reconnect}"));
        }
        CommandKind::Shutdown | CommandKind::Reboot => {
            let delay = delay_seconds
                .trim()
                .parse::<u64>()
                .unwrap_or_else(|_| default_delay_seconds());
            lines.push(format!("delay_seconds={delay}"));
        }
        CommandKind::UninstallClient => {
            lines.push(format!("remove_binary={remove_binary}"));
        }
        _ => {}
    }
    lines.join("\n")
}

fn default_delay_seconds() -> u64 {
    30
}

fn risk_text(command: &CommandKind) -> &'static str {
    match command {
        CommandKind::UpdateClient => "Restarts the remote client process.",
        CommandKind::UninstallClient => "Removes local client identity and stops the client.",
        CommandKind::KillClientProcess => "Stops the remote client process.",
        CommandKind::Shutdown => "Powers off the remote computer.",
        CommandKind::Reboot => "Restarts the remote computer.",
        CommandKind::ClientConfig => "Writes the remote client's config file.",
        CommandKind::DeleteClient => "Removes this client identity and stops the client.",
        _ => "",
    }
}

fn sanitize_single_line(value: &str) -> String {
    value.replace(['\t', '\r', '\n'], " ").trim().to_string()
}

fn identity_title(hostname: &str, username: &str) -> String {
    match (hostname.trim(), username.trim()) {
        ("", "") => "unknown-host".to_string(),
        (host, "") => host.to_string(),
        ("", user) => user.to_string(),
        (host, user) => format!("{host} / {user}"),
    }
}

fn command_title(command: &CommandKind) -> String {
    command
        .as_str()
        .split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::payload_for;
    use rdl_protocol::CommandKind;

    #[test]
    fn update_payload_omits_empty_path() {
        let payload = payload_for(&CommandKind::UpdateClient, "30", "", "", "", false, true);

        assert_eq!(payload, "confirm=true");
    }

    #[test]
    fn shutdown_payload_includes_delay() {
        let payload = payload_for(&CommandKind::Shutdown, "45", "", "", "", false, true);

        assert_eq!(payload, "confirm=true\ndelay_seconds=45");
    }

    #[test]
    fn uninstall_payload_includes_binary_choice() {
        let payload = payload_for(&CommandKind::UninstallClient, "30", "", "", "", true, true);

        assert_eq!(payload, "confirm=true\nremove_binary=true");
    }

    #[test]
    fn client_config_payload_includes_endpoint_and_reconnect() {
        let payload = payload_for(
            &CommandKind::ClientConfig,
            "30",
            "",
            "10.0.0.8",
            "7000",
            false,
            true,
        );

        assert_eq!(
            payload,
            "confirm=true\nip=10.0.0.8\nport=7000\nreconnect=true"
        );
    }
}
