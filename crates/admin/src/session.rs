use crate::{
    i18n::{self, t},
    windowing,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use eframe::egui;
use rdl_protocol::CommandKind;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

const TOOLBAR_CONTROL_HEIGHT: f32 = crate::theme::CONTROL_HEIGHT;

pub(crate) struct SessionCommandWindow {
    pub(crate) client_id: String,
    hostname: String,
    username: String,
    command: CommandKind,
    delay_seconds: Arc<Mutex<String>>,
    update_path: Arc<Mutex<String>>,
    client_config_ip: Arc<Mutex<String>>,
    client_config_port: Arc<Mutex<String>>,
    client_config_auth_token: Arc<Mutex<String>>,
    client_config_default_auth_token: Arc<Mutex<String>>,
    client_config_detail: Arc<Mutex<String>>,
    client_config_file: Arc<Mutex<String>>,
    client_config_status: Arc<Mutex<String>>,
    remove_binary: Arc<AtomicBool>,
    client_config_restart: Arc<AtomicBool>,
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
    default_auth_token: &str,
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
            if let Ok(mut value) = window.client_config_auth_token.lock() {
                value.clear();
            }
            if let Ok(mut value) = window.client_config_default_auth_token.lock() {
                *value = default_auth_token.to_string();
            }
            if let Ok(mut value) = window.client_config_detail.lock() {
                value.clear();
            }
            if let Ok(mut value) = window.client_config_file.lock() {
                value.clear();
            }
            if let Ok(mut value) = window.client_config_status.lock() {
                value.clear();
            }
            window.client_config_restart.store(true, Ordering::Relaxed);
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
        client_config_auth_token: Arc::new(Mutex::new(String::new())),
        client_config_default_auth_token: Arc::new(Mutex::new(default_auth_token.to_string())),
        client_config_detail: Arc::new(Mutex::new(String::new())),
        client_config_file: Arc::new(Mutex::new(String::new())),
        client_config_status: Arc::new(Mutex::new(String::new())),
        remove_binary: Arc::new(AtomicBool::new(false)),
        client_config_restart: Arc::new(AtomicBool::new(true)),
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
        let builder = if window.command == CommandKind::ClientConfig {
            windowing::child_viewport_builder(title, [660.0, 680.0], [540.0, 500.0])
        } else {
            windowing::child_viewport_builder(title, [520.0, 360.0], [420.0, 280.0])
        };

        let command = window.command.clone();
        let delay_seconds = window.delay_seconds.clone();
        let update_path = window.update_path.clone();
        let client_config_ip = window.client_config_ip.clone();
        let client_config_port = window.client_config_port.clone();
        let client_config_auth_token = window.client_config_auth_token.clone();
        let client_config_default_auth_token = window.client_config_default_auth_token.clone();
        let client_config_detail = window.client_config_detail.clone();
        let client_config_file = window.client_config_file.clone();
        let client_config_status = window.client_config_status.clone();
        let remove_binary = window.remove_binary.clone();
        let client_config_restart = window.client_config_restart.clone();
        let confirmed = window.confirmed.clone();
        let close_requested = window.close_requested.clone();
        let send_requested = window.send_requested.clone();

        ctx.show_viewport_immediate(viewport_id, builder, move |ui, _class| {
            if ui.ctx().input(|input| input.viewport().close_requested()) {
                close_requested.store(true, Ordering::Relaxed);
            }
            egui::CentralPanel::default()
                .frame(crate::theme::page_frame())
                .show_inside(ui, |ui| {
                    windowing::render_child_window_controls(ui);
                    render_form(
                        ui,
                        &command,
                        &delay_seconds,
                        &update_path,
                        &client_config_ip,
                        &client_config_port,
                        &client_config_auth_token,
                        &client_config_default_auth_token,
                        &client_config_detail,
                        &client_config_file,
                        &client_config_status,
                        &remove_binary,
                        &client_config_restart,
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
            let client_config_auth_token = window
                .client_config_auth_token
                .lock()
                .map(|value| value.clone())
                .unwrap_or_default();
            let client_config_file = window
                .client_config_file
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
                    &client_config_auth_token,
                    &client_config_file,
                    window.remove_binary.load(Ordering::Relaxed),
                    window.client_config_restart.load(Ordering::Relaxed),
                ),
            });
            if window.command == CommandKind::ClientConfig {
                window.confirmed.store(false, Ordering::Relaxed);
                if let Ok(mut status) = window.client_config_status.lock() {
                    *status = format!("status=sending\nmessage={}", t("Saving client config..."));
                }
            } else {
                window.open = false;
            }
        }
    }

    windows.retain(|window| window.open);
    outbound
}

pub(crate) fn handle_client_config_ack(
    windows: &mut [SessionCommandWindow],
    client_id: &str,
    accepted: bool,
    detail: &str,
) -> bool {
    let Some(window) = windows.iter_mut().find(|window| {
        window.client_id == client_id && window.command == CommandKind::ClientConfig
    }) else {
        return false;
    };
    let status = payload_field(detail, "status");
    if status.as_deref() != Some("current") && status.as_deref() != Some("updated") && accepted {
        return false;
    }

    if let Ok(mut value) = window.client_config_status.lock() {
        *value = detail.to_string();
    }
    if accepted && status.as_deref() == Some("current") {
        if let Ok(mut value) = window.client_config_detail.lock() {
            *value = detail.to_string();
        }
        if let Some(file) = decode_detail_base64(detail, "config_file_b64") {
            if let Ok(mut value) = window.client_config_file.lock() {
                *value = file;
            }
        }
    }
    true
}

fn render_form(
    ui: &mut egui::Ui,
    command: &CommandKind,
    delay_seconds: &Arc<Mutex<String>>,
    update_path: &Arc<Mutex<String>>,
    client_config_ip: &Arc<Mutex<String>>,
    client_config_port: &Arc<Mutex<String>>,
    client_config_auth_token: &Arc<Mutex<String>>,
    client_config_default_auth_token: &Arc<Mutex<String>>,
    client_config_detail: &Arc<Mutex<String>>,
    client_config_file: &Arc<Mutex<String>>,
    client_config_status: &Arc<Mutex<String>>,
    remove_binary: &Arc<AtomicBool>,
    client_config_restart: &Arc<AtomicBool>,
    confirmed: &Arc<AtomicBool>,
    send_requested: &Arc<AtomicBool>,
) {
    crate::theme::panel_frame()
        .corner_radius(8.0)
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(
                    crate::theme::danger_text(risk_text(command))
                        .size(13.0)
                        .strong(),
                );
                ui.add_space(10.0);
                if command == &CommandKind::ClientConfig {
                    let max_height = (ui.available_height() - 104.0).max(220.0);
                    egui::ScrollArea::vertical()
                        .id_salt("client_config_fields")
                        .auto_shrink([false, false])
                        .max_height(max_height)
                        .show(ui, |ui| {
                            render_command_fields(
                                ui,
                                command,
                                delay_seconds,
                                update_path,
                                client_config_ip,
                                client_config_port,
                                client_config_auth_token,
                                client_config_default_auth_token,
                                client_config_detail,
                                client_config_file,
                                client_config_status,
                                remove_binary,
                                client_config_restart,
                            );
                        });
                } else {
                    render_command_fields(
                        ui,
                        command,
                        delay_seconds,
                        update_path,
                        client_config_ip,
                        client_config_port,
                        client_config_auth_token,
                        client_config_default_auth_token,
                        client_config_detail,
                        client_config_file,
                        client_config_status,
                        remove_binary,
                        client_config_restart,
                    );
                }
                ui.add_space(10.0);
                let (send_enabled, disabled_reason) = if command == &CommandKind::ClientConfig {
                    let detail = client_config_detail
                        .lock()
                        .map(|value| value.clone())
                        .unwrap_or_default();
                    client_config_action_state(&detail, &client_config_status)
                } else {
                    (true, None)
                };
                render_confirm(
                    ui,
                    command,
                    confirmed,
                    send_requested,
                    send_enabled,
                    disabled_reason,
                );
                if command == &CommandKind::ClientConfig {
                    let detail = client_config_detail
                        .lock()
                        .map(|value| value.clone())
                        .unwrap_or_default();
                    ui.add_space(crate::theme::SECTION_GAP);
                    render_client_config_status_bar(ui, &detail, client_config_status);
                }
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
    client_config_auth_token: &Arc<Mutex<String>>,
    client_config_default_auth_token: &Arc<Mutex<String>>,
    client_config_detail: &Arc<Mutex<String>>,
    client_config_file: &Arc<Mutex<String>>,
    client_config_status: &Arc<Mutex<String>>,
    remove_binary: &Arc<AtomicBool>,
    client_config_restart: &Arc<AtomicBool>,
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
                client_config_auth_token,
                client_config_default_auth_token,
                client_config_detail,
                client_config_file,
                client_config_status,
                client_config_restart,
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
                egui::RichText::new(t(
                    "The client will disconnect after acknowledging the command.",
                ))
                .size(12.0)
                .color(crate::theme::palette().muted),
            );
        }
        _ => {}
    }
}

fn render_client_config(
    ui: &mut egui::Ui,
    _client_config_ip: &Arc<Mutex<String>>,
    _client_config_port: &Arc<Mutex<String>>,
    _client_config_auth_token: &Arc<Mutex<String>>,
    _client_config_default_auth_token: &Arc<Mutex<String>>,
    client_config_detail: &Arc<Mutex<String>>,
    client_config_file: &Arc<Mutex<String>>,
    _client_config_status: &Arc<Mutex<String>>,
    client_config_restart: &Arc<AtomicBool>,
) {
    let detail = client_config_detail
        .lock()
        .map(|value| value.clone())
        .unwrap_or_default();
    let form_enabled = !detail.trim().is_empty() && client_config_editable(&detail);

    client_config_restart.store(true, Ordering::Relaxed);
    render_client_config_detail(ui, &detail, client_config_file, form_enabled);
}

fn render_client_config_detail(
    ui: &mut egui::Ui,
    detail: &str,
    client_config_file: &Arc<Mutex<String>>,
    editable: bool,
) {
    if detail.trim().is_empty() {
        ui.label(crate::theme::muted_text(t("Loading client config...")));
        return;
    }

    crate::theme::panel_frame_with_margin(crate::theme::PANEL_MARGIN).show(ui, |ui| {
        config_summary_row(
            ui,
            t("Embedded Config"),
            client_config_embedded_label(detail),
        );
        config_summary_row(
            ui,
            t("Runtime Config Path"),
            payload_field(detail, "runtime_config_path").unwrap_or_default(),
        );
    });
    ui.add_space(crate::theme::SECTION_GAP);

    let startup_args = decode_detail_base64(detail, "startup_args_b64").unwrap_or_default();
    let startup_args = sanitize_single_line(&startup_args);
    let startup_args = if startup_args.is_empty() {
        t("No startup arguments").to_string()
    } else {
        startup_args
    };
    render_copyable_single_line_block(ui, t("Startup Arguments"), &startup_args);
    ui.add_space(crate::theme::SECTION_GAP);

    render_config_file_editor(ui, client_config_file, editable);
}

fn config_summary_row(ui: &mut egui::Ui, label: &str, value: impl Into<String>) {
    ui.horizontal_wrapped(|ui| {
        ui.add_sized(
            [150.0, 18.0],
            egui::Label::new(crate::theme::muted_text(label)),
        );
        ui.label(crate::theme::body_text(value.into()));
    });
}

fn render_copyable_single_line_block(ui: &mut egui::Ui, title: &str, text: &str) {
    crate::theme::panel_frame_with_margin(crate::theme::PANEL_MARGIN).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(crate::theme::muted_text(title).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(t("Copy")).clicked() {
                    ui.ctx().copy_text(text.to_string());
                }
            });
        });
        ui.add_space(crate::theme::SECTION_GAP);
        egui::ScrollArea::horizontal()
            .id_salt((title, "single_line"))
            .auto_shrink([false, true])
            .max_height(TOOLBAR_CONTROL_HEIGHT)
            .show(ui, |ui| {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(text)
                            .monospace()
                            .size(12.0)
                            .color(crate::theme::palette().text),
                    )
                    .wrap_mode(egui::TextWrapMode::Extend),
                );
            });
    });
}

fn render_config_file_editor(
    ui: &mut egui::Ui,
    client_config_file: &Arc<Mutex<String>>,
    editable: bool,
) {
    crate::theme::panel_frame_with_margin(crate::theme::PANEL_MARGIN).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(crate::theme::muted_text(t("Config File Content")).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let text = client_config_file
                    .lock()
                    .map(|value| value.clone())
                    .unwrap_or_default();
                if ui.button(t("Copy")).clicked() {
                    ui.ctx().copy_text(text);
                }
            });
        });
        ui.add_space(crate::theme::SECTION_GAP);

        let mut text = client_config_file
            .lock()
            .map(|value| value.clone())
            .unwrap_or_default();
        let rows = text.lines().count().clamp(10, 18);
        let response = ui.add_sized(
            [ui.available_width(), rows as f32 * 18.0 + 16.0],
            egui::TextEdit::multiline(&mut text)
                .font(egui::TextStyle::Monospace)
                .desired_width(f32::INFINITY)
                .desired_rows(rows)
                .interactive(editable),
        );
        if editable && response.changed() {
            if let Ok(mut value) = client_config_file.lock() {
                *value = text;
            }
        }
    });
}

fn decode_detail_base64(detail: &str, key: &str) -> Option<String> {
    payload_field(detail, key).and_then(|value| {
        STANDARD
            .decode(value)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
    })
}

fn render_update_path(ui: &mut egui::Ui, update_path: &Arc<Mutex<String>>) {
    let mut value = update_path
        .lock()
        .map(|value| value.clone())
        .unwrap_or_default();
    ui.label(
        egui::RichText::new(t("Replacement Binary Path"))
            .size(12.0)
            .color(crate::theme::palette().muted),
    );
    let response = ui.add_sized(
        [ui.available_width(), TOOLBAR_CONTROL_HEIGHT],
        egui::TextEdit::singleline(&mut value)
            .hint_text(t("Optional path on the client"))
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
        egui::RichText::new(t("Delay Seconds"))
            .size(12.0)
            .color(crate::theme::palette().muted),
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
        .checkbox(&mut value, t("Remove client binary after exit"))
        .changed()
    {
        remove_binary.store(value, Ordering::Relaxed);
    }
}

fn render_confirm(
    ui: &mut egui::Ui,
    command: &CommandKind,
    confirmed: &Arc<AtomicBool>,
    send_requested: &Arc<AtomicBool>,
    command_enabled: bool,
    disabled_reason: Option<&'static str>,
) {
    if !command_enabled {
        confirmed.store(false, Ordering::Relaxed);
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().interact_size.y = TOOLBAR_CONTROL_HEIGHT;
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let action_label = confirm_action_label(command);
            if ui
                .add_enabled(command_enabled, egui::Button::new(action_label))
                .clicked()
            {
                confirmed.store(true, Ordering::Relaxed);
            }
            if let Some(reason) = disabled_reason {
                ui.label(
                    egui::RichText::new(t(reason))
                        .size(12.0)
                        .color(crate::theme::palette().warn),
                );
            }
        });
    });
    if confirmed.load(Ordering::Relaxed) && command_enabled {
        render_confirm_dialog(ui, command, confirmed, send_requested);
    }
}

fn render_confirm_dialog(
    ui: &mut egui::Ui,
    command: &CommandKind,
    confirmed: &Arc<AtomicBool>,
    send_requested: &Arc<AtomicBool>,
) {
    egui::Window::new(confirm_title(command))
        .collapsible(false)
        .resizable(false)
        .default_width(460.0)
        .show(ui.ctx(), |ui| {
            ui.label(
                egui::RichText::new(confirm_message(command))
                    .size(12.0)
                    .color(crate::theme::palette().muted),
            );
            let risk = risk_text(command);
            if !risk.trim().is_empty() && risk != confirm_message(command) {
                ui.label(
                    egui::RichText::new(risk)
                        .size(12.0)
                        .color(crate::theme::palette().text),
                );
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new(confirm_action_label(command))
                            .color(confirm_action_color(command))
                            .strong(),
                    ))
                    .clicked()
                {
                    send_requested.store(true, Ordering::Relaxed);
                    confirmed.store(false, Ordering::Relaxed);
                    ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
                }
                if ui.button(t("Cancel")).clicked() {
                    confirmed.store(false, Ordering::Relaxed);
                }
            });
        });
}

fn confirm_title(command: &CommandKind) -> &'static str {
    if command == &CommandKind::ClientConfig {
        t("Confirm Save")
    } else {
        t("Confirm")
    }
}

fn confirm_message(command: &CommandKind) -> &'static str {
    if command == &CommandKind::ClientConfig {
        t("Save this client config and restart the client?")
    } else {
        let risk = risk_text(command);
        if risk.trim().is_empty() {
            t("Send this command to the client?")
        } else {
            risk
        }
    }
}

fn confirm_action_label(command: &CommandKind) -> &'static str {
    if command == &CommandKind::ClientConfig {
        t("Save")
    } else {
        t("Send")
    }
}

fn confirm_action_color(command: &CommandKind) -> egui::Color32 {
    if command == &CommandKind::ClientConfig {
        crate::theme::palette().good
    } else {
        crate::theme::palette().bad
    }
}

fn payload_for(
    command: &CommandKind,
    delay_seconds: &str,
    update_path: &str,
    _client_config_ip: &str,
    _client_config_port: &str,
    _client_config_auth_token: &str,
    client_config_file: &str,
    remove_binary: bool,
    _client_config_restart: bool,
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
            lines.push(format!(
                "config_file_b64={}",
                STANDARD.encode(client_config_file)
            ));
            lines.push("restart=true".to_string());
            lines.push("reconnect=true".to_string());
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
        CommandKind::UpdateClient => t("Restarts the remote client process."),
        CommandKind::UninstallClient => t("Removes local client identity and stops the client."),
        CommandKind::KillClientProcess => t("Stops the remote client process."),
        CommandKind::Shutdown => t("Powers off the remote computer."),
        CommandKind::Reboot => t("Restarts the remote computer."),
        CommandKind::ClientConfig => {
            t("Writes the remote client's config file and restarts it from that file.")
        }
        CommandKind::DeleteClient => t("Removes this client identity and stops the client."),
        _ => "",
    }
}

fn sanitize_single_line(value: &str) -> String {
    value.replace(['\t', '\r', '\n'], " ").trim().to_string()
}

fn client_config_action_state(
    detail: &str,
    client_config_status: &Arc<Mutex<String>>,
) -> (bool, Option<&'static str>) {
    if detail.trim().is_empty() {
        return (
            false,
            Some("Waiting for client config snapshot before changes are enabled."),
        );
    }
    if !client_config_editable(detail) {
        return (false, Some("Builder/embedded clients are read-only."));
    }
    let sending = client_config_status
        .lock()
        .ok()
        .and_then(|status| payload_field(&status, "status"))
        .as_deref()
        == Some("sending");
    if sending {
        return (false, Some("Saving client config..."));
    }
    (true, None)
}

fn client_config_embedded_label(detail: &str) -> &'static str {
    if payload_field(detail, "embedded_config")
        .map(|value| detail_bool(&value))
        .unwrap_or(false)
    {
        t("yes")
    } else {
        t("no")
    }
}

fn render_client_config_status_bar(
    ui: &mut egui::Ui,
    detail: &str,
    client_config_status: &Arc<Mutex<String>>,
) {
    let status = client_config_status
        .lock()
        .map(|value| value.clone())
        .unwrap_or_default();
    let (label, notice, color) = client_config_status_state(detail, &status);
    crate::theme::status_frame().show(ui, |ui| {
        ui.set_min_height(26.0);
        crate::theme::render_status_line(ui, label, color, &notice, |_| {});
    });
}

fn client_config_status_state(detail: &str, status: &str) -> (&'static str, String, egui::Color32) {
    let palette = crate::theme::palette();
    let status_key = payload_field(status, "status");
    let message = client_config_status_notice(
        status_key.as_deref(),
        payload_field(status, "message").as_deref(),
        detail,
    );
    match status_key.as_deref() {
        Some("sending") => (t("Sending"), message, palette.warn),
        Some("current") => (t("Ready"), message, palette.muted),
        Some("updated") => (t("Done"), message, palette.good),
        Some("error") | Some("refused") => (t("Failed"), message, palette.bad),
        _ if detail.trim().is_empty() => (t("Pending"), message, palette.warn),
        _ if !client_config_editable(detail) => (t("Read-only"), message, palette.warn),
        _ => (t("Ready"), message, palette.muted),
    }
}

fn client_config_status_notice(
    status_key: Option<&str>,
    raw_message: Option<&str>,
    detail: &str,
) -> String {
    match status_key {
        Some("current") => t("Client config loaded.").to_string(),
        Some("sending") => t("Saving client config...").to_string(),
        Some("updated") => t("Client config saved. Restarting client.").to_string(),
        Some("error") | Some("refused") => raw_message
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| t("Command failed"))
            .to_string(),
        _ if detail.trim().is_empty() => {
            t("Waiting for client config snapshot before changes are enabled.").to_string()
        }
        _ if !client_config_editable(detail) => {
            t("Builder/embedded clients are read-only.").to_string()
        }
        _ => t("Ready").to_string(),
    }
}

fn client_config_editable(detail: &str) -> bool {
    payload_field(detail, "config_editable")
        .map(|value| detail_bool(&value))
        .unwrap_or(true)
}

fn detail_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn payload_field(payload: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    payload
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .map(|value| value.trim().to_string())
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
    i18n::command_title(command).to_string()
}
