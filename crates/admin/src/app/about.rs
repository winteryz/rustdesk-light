use super::ui::about_row;
use super::*;

const PACKAGE_AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
const PACKAGE_REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
const PACKAGE_LICENSE: &str = env!("CARGO_PKG_LICENSE");
const FALLBACK_REPOSITORY: &str = "https://github.com/marlkiller/rust-desk-light";

fn package_repository() -> &'static str {
    if PACKAGE_REPOSITORY.trim().is_empty() {
        FALLBACK_REPOSITORY
    } else {
        PACKAGE_REPOSITORY
    }
}

impl AdminApp {
    pub(super) fn render_about_window(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            return;
        }

        let mut open = self.about_open;
        let mut close_requested = false;
        egui::Window::new(t("About"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new("rust-desk-light admin")
                        .size(18.0)
                        .color(crate::theme::palette().text)
                        .strong(),
                );
                ui.add_space(6.0);
                about_row(ui, t("Version"), rdl_version::display_version());
                about_row(ui, t("Author"), PACKAGE_AUTHORS);
                about_row(ui, t("Repository"), package_repository());
                about_row(ui, t("License"), PACKAGE_LICENSE);
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button(t("Copy repository")).clicked() {
                        ui.ctx().copy_text(package_repository().to_string());
                    }
                    if ui.button(t("Close")).clicked() {
                        close_requested = true;
                    }
                });
            });
        self.about_open = open && !close_requested;
    }
}
