use super::ui;
use crate::i18n::t;
use base64::{engine::general_purpose::STANDARD, Engine};
use eframe::egui;
use std::sync::{atomic::AtomicBool, Arc, Mutex};

const TRIGGER_STARTUP: &str = "startup";
const TRIGGER_DAILY: &str = "daily";

pub(super) fn render(
    ui: &mut egui::Ui,
    task_name: &Arc<Mutex<String>>,
    task_command: &Arc<Mutex<String>>,
    task_trigger: &Arc<Mutex<String>>,
    task_time: &Arc<Mutex<String>>,
    send_requested: &Arc<AtomicBool>,
) {
    ui::render_text_field(ui, t("Task Name"), task_name, "rdl-task");
    ui.add_space(crate::theme::SECTION_GAP);
    ui::render_text_field(
        ui,
        t("Command"),
        task_command,
        t("Command or executable path"),
    );
    ui.add_space(crate::theme::SECTION_GAP);
    render_trigger(ui, task_trigger, task_time);
    ui.add_space(crate::theme::PANEL_MARGIN);

    let name_missing = task_name
        .lock()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true);
    let command_missing = task_command
        .lock()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true);
    let time_invalid = task_trigger
        .lock()
        .map(|value| value.as_str() == TRIGGER_DAILY)
        .unwrap_or(false)
        && task_time
            .lock()
            .map(|value| !valid_hhmm(&value))
            .unwrap_or(true);
    let disabled_message = if name_missing {
        t("Task name is required")
    } else if command_missing {
        t("Command is required")
    } else if time_invalid {
        t("Time must be HH:MM")
    } else {
        ""
    };
    ui::render_run_button(
        ui,
        disabled_message.is_empty(),
        disabled_message,
        send_requested,
    );
}

fn render_trigger(
    ui: &mut egui::Ui,
    task_trigger: &Arc<Mutex<String>>,
    task_time: &Arc<Mutex<String>>,
) {
    let mut selected = task_trigger
        .lock()
        .map(|value| value.clone())
        .unwrap_or_else(|_| TRIGGER_STARTUP.to_string());
    if selected.is_empty() {
        selected = TRIGGER_STARTUP.to_string();
    }

    ui.horizontal(|ui| {
        ui.spacing_mut().interact_size.y = ui::TOOLBAR_CONTROL_HEIGHT;
        ui::render_inline_label(ui, t("Trigger"));
        egui::ComboBox::from_id_salt(("create_task_trigger", Arc::as_ptr(task_trigger)))
            .width(180.0)
            .selected_text(trigger_label(&selected))
            .show_ui(ui, |ui| {
                for (value, label) in trigger_options() {
                    if ui.selectable_label(selected == value, label).clicked() {
                        selected = value.to_string();
                        if let Ok(mut target) = task_trigger.lock() {
                            *target = selected.clone();
                        }
                    }
                }
            });
    });

    if selected == TRIGGER_DAILY {
        ui.add_space(crate::theme::SECTION_GAP);
        ui::render_inline_text_field(ui, t("Start Time"), task_time, "09:00");
    }
}

pub(super) fn payload_for(name: &str, command: &str, trigger: &str, time: &str) -> String {
    let trigger = if trigger.trim() == TRIGGER_DAILY {
        TRIGGER_DAILY
    } else {
        TRIGGER_STARTUP
    };
    let mut lines = vec![
        "action=create".to_string(),
        format!("name={}", sanitize_single_line(name)),
        format!("trigger={trigger}"),
        format!("command_b64={}", STANDARD.encode(command)),
    ];
    if trigger == TRIGGER_DAILY {
        lines.push(format!("time={}", sanitize_single_line(time)));
    }
    lines.join("\n")
}

fn trigger_options() -> [(&'static str, &'static str); 2] {
    [
        (TRIGGER_STARTUP, t("At startup")),
        (TRIGGER_DAILY, t("Daily")),
    ]
}

fn trigger_label(value: &str) -> &'static str {
    match value {
        TRIGGER_DAILY => t("Daily"),
        _ => t("At startup"),
    }
}

fn sanitize_single_line(value: &str) -> String {
    value.replace(['\t', '\r', '\n'], " ").trim().to_string()
}

fn valid_hhmm(value: &str) -> bool {
    let Some((hour, minute)) = value.trim().split_once(':') else {
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
