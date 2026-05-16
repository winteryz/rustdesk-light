use eframe::egui;
use std::sync::Arc;

pub(super) const COLOR_BG: egui::Color32 = egui::Color32::from_rgb(246, 248, 251);
pub(super) const COLOR_PANEL: egui::Color32 = egui::Color32::from_rgb(255, 255, 255);
pub(super) const COLOR_BORDER: egui::Color32 = egui::Color32::from_rgb(222, 228, 236);
pub(super) const COLOR_TEXT: egui::Color32 = egui::Color32::from_rgb(24, 33, 47);
pub(super) const COLOR_MUTED: egui::Color32 = egui::Color32::from_rgb(96, 108, 124);
pub(super) const COLOR_ACCENT: egui::Color32 = egui::Color32::from_rgb(35, 99, 188);
pub(super) const COLOR_GOOD: egui::Color32 = egui::Color32::from_rgb(24, 135, 84);
pub(super) const COLOR_BAD: egui::Color32 = egui::Color32::from_rgb(190, 58, 58);
pub(super) const COLOR_WARN: egui::Color32 = egui::Color32::from_rgb(179, 116, 28);
pub(super) const TOOLBAR_CONTROL_HEIGHT: f32 = 28.0;
const ACTIVITY_LOG_LIMIT: usize = 300;

pub(super) fn apply_admin_theme(ctx: &egui::Context) {
    install_cjk_font(ctx);

    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);
    style.visuals = egui::Visuals::light();
    style.visuals.window_fill = COLOR_PANEL;
    style.visuals.panel_fill = COLOR_BG;
    style.visuals.widgets.noninteractive.fg_stroke.color = COLOR_TEXT;
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(238, 242, 247);
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(226, 234, 244);
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(216, 228, 242);
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(216, 232, 252);
    style.visuals.selection.stroke.color = COLOR_ACCENT;
    ctx.set_global_style(style);
}

fn install_cjk_font(ctx: &egui::Context) {
    let Some(font_bytes) = load_system_cjk_font() else {
        return;
    };

    let mut fonts = egui::FontDefinitions::default();
    let font_name = "rdl_cjk_fallback".to_string();
    fonts.font_data.insert(
        font_name.clone(),
        Arc::new(egui::FontData::from_owned(font_bytes)),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, font_name.clone());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push(font_name);
    ctx.set_fonts(fonts);
}

fn load_system_cjk_font() -> Option<Vec<u8>> {
    let candidates = [
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\msyh.ttf",
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\simsun.ttc",
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
    ];

    candidates.iter().find_map(|path| std::fs::read(path).ok())
}

pub(super) fn panel(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::default()
        .fill(COLOR_PANEL)
        .stroke(egui::Stroke::new(1.0, COLOR_BORDER))
        .corner_radius(8.0)
        .inner_margin(14.0)
        .show(ui, add_contents);
}

pub(super) fn section_title(ui: &mut egui::Ui, title: &str) {
    ui.label(
        egui::RichText::new(title)
            .size(14.0)
            .color(COLOR_TEXT)
            .strong(),
    );
}

pub(super) fn table_header(ui: &mut egui::Ui, title: &str) {
    ui.label(
        egui::RichText::new(title)
            .size(12.0)
            .color(COLOR_MUTED)
            .strong(),
    );
}

pub(super) fn centered_cell(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    ui.with_layout(
        egui::Layout::left_to_right(egui::Align::Center),
        add_contents,
    );
}

pub(super) fn cell_label(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.add(
        egui::Label::new(egui::RichText::new(text).size(12.0))
            .selectable(false)
            .sense(egui::Sense::hover()),
    );
}

pub(super) fn connection_status_pill(ui: &mut egui::Ui, connected: bool) {
    let (text, color) = if connected {
        ("Online", COLOR_GOOD)
    } else {
        ("Reconnecting", COLOR_BAD)
    };
    status_badge(ui, text, color);
}

pub(super) fn timestamped_log(line: impl Into<String>) -> String {
    format!("[{}] {}", activity_time_label(), line.into())
}

pub(super) fn prune_activity_logs(log_lines: &mut Vec<String>) {
    if log_lines.len() > ACTIVITY_LOG_LIMIT {
        log_lines.drain(0..log_lines.len() - ACTIVITY_LOG_LIMIT);
    }
}

pub(super) fn activity_context_menu(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    id: egui::Id,
    log_lines: &mut Vec<String>,
) {
    ui.interact(rect, id.with("activity_context_menu"), egui::Sense::click())
        .context_menu(|ui| {
            if ui.button("Copy").clicked() {
                ui.ctx().copy_text(log_lines.join("\n"));
                ui.close();
            }
            if ui.button("Clear").clicked() {
                log_lines.clear();
                ui.close();
            }
        });
}

fn activity_time_label() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let china_time = now + 8 * 60 * 60;
    let seconds_today = china_time % (24 * 60 * 60);
    let hour = seconds_today / 3600;
    let minute = (seconds_today % 3600) / 60;
    let second = seconds_today % 60;
    format!("{hour:02}:{minute:02}:{second:02}")
}

fn status_badge(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    egui::Frame::default()
        .fill(color.gamma_multiply(0.10))
        .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.35)))
        .corner_radius(999.0)
        .inner_margin(egui::Margin::symmetric(12, 6))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).color(color).strong());
        });
}

pub(super) fn compact_id(value: &str) -> String {
    if value.len() > 22 {
        format!("{}...", &value[..22])
    } else {
        value.to_string()
    }
}

pub(super) fn last_seen_label(last_seen_epoch_ms: u128) -> String {
    if last_seen_epoch_ms == 0 {
        return "Never".to_string();
    }
    format_epoch_utc(last_seen_epoch_ms / 1000)
}

fn format_epoch_utc(epoch_seconds: u128) -> String {
    let seconds = epoch_seconds as i64;
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02} UTC")
}

fn civil_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let days = days_since_epoch + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_param = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_param + 2) / 5 + 1;
    let month = month_param + if month_param < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

pub(super) fn metric(ui: &mut egui::Ui, label: &str, value: String) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).color(COLOR_MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(value).color(COLOR_TEXT).strong());
        });
    });
}

pub(super) fn empty_state(ui: &mut egui::Ui) {
    ui.add_space(48.0);
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new("No clients online")
                .size(16.0)
                .color(COLOR_TEXT),
        );
        ui.label(
            egui::RichText::new("Start a client or refresh after it connects.")
                .size(13.0)
                .color(COLOR_MUTED),
        );
    });
    ui.add_space(48.0);
}
