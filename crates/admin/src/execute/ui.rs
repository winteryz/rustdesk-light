use crate::i18n::t;
use eframe::egui;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

pub(super) const TOOLBAR_CONTROL_HEIGHT: f32 = crate::theme::CONTROL_HEIGHT;
const INLINE_LABEL_WIDTH: f32 = 86.0;
pub(super) const CODE_ROW_HEIGHT: f32 = 18.0;
const STATUS_BAR_HEIGHT: f32 = 44.0;

pub(super) fn render_status_panel(ui: &mut egui::Ui, result_status: &Arc<Mutex<String>>) {
    egui::Panel::bottom(egui::Id::new((
        "execute_status_panel",
        Arc::as_ptr(result_status),
    )))
    .exact_size(STATUS_BAR_HEIGHT)
    .show_separator_line(false)
    .frame(crate::theme::status_frame())
    .show_inside(ui, |ui| render_status_bar(ui, result_status));
}

fn render_status_bar(ui: &mut egui::Ui, result_status: &Arc<Mutex<String>>) {
    let status = result_status
        .lock()
        .map(|value| value.clone())
        .unwrap_or_default();
    let (label, notice, color) = status_bar_state(&status);

    ui.set_min_height(26.0);
    crate::theme::render_status_line(ui, &label, color, &notice, |_| {});
}

fn status_bar_state(status: &str) -> (String, String, egui::Color32) {
    let status = status.trim();
    let palette = crate::theme::palette();
    if status.is_empty() || status == t("Ready") {
        return (
            t("Ready").to_string(),
            t("Ready").to_string(),
            palette.muted,
        );
    }
    if status == t("Running...") || status == t("Running") {
        return (
            t("Running").to_string(),
            t("Waiting for client result").to_string(),
            palette.warn,
        );
    }
    if status == t("Rejected") || status.starts_with(&format!("{}:", t("Rejected"))) {
        return (
            t("Rejected").to_string(),
            status_notice(status, t("Rejected"), t("Command failed")),
            palette.bad,
        );
    }
    if status == t("Failed") || status.starts_with(&format!("{}:", t("Failed"))) {
        return (
            t("Failed").to_string(),
            status_notice(status, t("Failed"), t("Command failed")),
            palette.bad,
        );
    }
    if status == t("Completed") || status == t("Done") {
        return (
            t("Done").to_string(),
            t("Result received").to_string(),
            palette.good,
        );
    }
    (
        status.to_string(),
        t("Result received").to_string(),
        palette.good,
    )
}

fn status_notice(status: &str, prefix: &str, fallback: &str) -> String {
    status
        .strip_prefix(prefix)
        .and_then(|value| value.trim_start().strip_prefix(':'))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| fallback.to_string())
}

pub(super) fn render_inline_label(ui: &mut egui::Ui, label: &str) {
    ui.allocate_ui_with_layout(
        egui::vec2(INLINE_LABEL_WIDTH, TOOLBAR_CONTROL_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.label(
                egui::RichText::new(label)
                    .size(12.0)
                    .color(crate::theme::palette().muted),
            );
        },
    );
}

pub(super) fn render_inline_text_field(
    ui: &mut egui::Ui,
    label: &str,
    value: &Arc<Mutex<String>>,
    hint: &str,
) {
    let mut text = value.lock().map(|value| value.clone()).unwrap_or_default();
    ui.horizontal(|ui| {
        ui.spacing_mut().interact_size.y = TOOLBAR_CONTROL_HEIGHT;
        render_inline_label(ui, label);
        let response = ui.add_sized(
            [ui.available_width(), TOOLBAR_CONTROL_HEIGHT],
            egui::TextEdit::singleline(&mut text)
                .hint_text(hint)
                .vertical_align(egui::Align::Center),
        );
        if response.changed() {
            if let Ok(mut value) = value.lock() {
                *value = text;
            }
        }
    });
}

pub(super) fn render_text_field(
    ui: &mut egui::Ui,
    label: &str,
    value: &Arc<Mutex<String>>,
    hint: &str,
) {
    let mut text = value.lock().map(|value| value.clone()).unwrap_or_default();
    ui.label(
        egui::RichText::new(label)
            .size(12.0)
            .color(crate::theme::palette().muted),
    );
    let response = ui.add_sized(
        [ui.available_width(), TOOLBAR_CONTROL_HEIGHT],
        egui::TextEdit::singleline(&mut text)
            .hint_text(hint)
            .vertical_align(egui::Align::Center),
    );
    if response.changed() {
        if let Ok(mut value) = value.lock() {
            *value = text;
        }
    }
}

pub(super) fn render_run_button(
    ui: &mut egui::Ui,
    can_run: bool,
    disabled_message: &str,
    send_requested: &Arc<AtomicBool>,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().interact_size.y = TOOLBAR_CONTROL_HEIGHT;
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(can_run, egui::Button::new(t("Run")))
                .clicked()
            {
                send_requested.store(true, Ordering::Relaxed);
                ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
            }
            if !can_run && !disabled_message.is_empty() {
                ui.label(
                    egui::RichText::new(disabled_message)
                        .size(12.0)
                        .color(crate::theme::palette().text),
                );
            }
        });
    });
}
