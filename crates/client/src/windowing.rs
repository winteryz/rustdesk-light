use eframe::egui;

pub(crate) fn child_viewport_builder(
    title: impl Into<String>,
    inner_size: [f32; 2],
    min_inner_size: [f32; 2],
) -> egui::ViewportBuilder {
    let builder = egui::ViewportBuilder::default()
        .with_title(title)
        .with_inner_size(inner_size)
        .with_min_inner_size(min_inner_size)
        .with_resizable(true);

    #[cfg(target_os = "macos")]
    {
        builder.with_fullscreen(false).with_maximize_button(false)
    }

    #[cfg(not(target_os = "macos"))]
    {
        builder
    }
}

pub(crate) fn render_child_window_controls(ui: &mut egui::Ui) {
    #[cfg(target_os = "macos")]
    {
        let (is_maximized, is_fullscreen) = ui.ctx().input(|input| {
            let viewport = input.viewport();
            (
                viewport.maximized.unwrap_or(false),
                viewport.fullscreen.unwrap_or(false),
            )
        });
        let is_expanded = is_maximized || is_fullscreen;

        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = if is_expanded { "Restore" } else { "Maximize" };
                if ui
                    .add(egui::Button::new(label).min_size(egui::vec2(88.0, 24.0)))
                    .clicked()
                {
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Maximized(!is_expanded));
                }
            });
        });
        ui.add_space(6.0);
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = ui;
    }
}
