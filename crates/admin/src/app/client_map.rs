use super::{ui, ClientRow};
use crate::windowing;
use eframe::egui;
use rdl_protocol::ClientInfo;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

pub(super) struct ClientMapWindow {
    open: bool,
    close_requested: Arc<AtomicBool>,
}

impl ClientMapWindow {
    pub(super) fn new() -> Self {
        Self {
            open: false,
            close_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(super) fn open(&mut self) {
        self.open = true;
        self.close_requested.store(false, Ordering::Relaxed);
    }

    pub(super) fn render(
        &mut self,
        ctx: &egui::Context,
        clients: &[ClientRow],
        selected_client_id: &mut Option<String>,
        client_filter: &mut String,
    ) {
        if self.close_requested.swap(false, Ordering::Relaxed) {
            self.open = false;
        }
        if !self.open {
            return;
        }

        let close_requested = self.close_requested.clone();
        let selected_sink = Arc::new(Mutex::new(None::<String>));
        let selected_out = selected_sink.clone();
        let selected_current = selected_client_id.clone();
        let clients = filtered_clients(clients, client_filter);
        let viewport_id = egui::ViewportId::from_hash_of("admin_client_map");
        let builder =
            windowing::child_viewport_builder("Client Map", [980.0, 660.0], [720.0, 520.0]);

        ctx.show_viewport_immediate(viewport_id, builder, |ui, _class| {
            if ui.ctx().input(|input| input.viewport().close_requested()) {
                close_requested.store(true, Ordering::Relaxed);
            }
            egui::CentralPanel::default()
                .frame(egui::Frame::default().fill(ui::COLOR_BG).inner_margin(12.0))
                .show_inside(ui, |ui| {
                    windowing::render_child_window_controls(ui);
                    render_map_contents(
                        ui,
                        &clients,
                        selected_current.as_deref(),
                        client_filter,
                        &selected_sink,
                    );
                });
        });

        if let Some(client_id) = selected_out.lock().ok().and_then(|value| value.clone()) {
            *selected_client_id = Some(client_id);
        }
    }
}

pub(super) fn client_location_label(client: &ClientInfo) -> String {
    client
        .location
        .as_ref()
        .map(|location| {
            if location.accuracy_meters >= 1_000 {
                format!(
                    "{} ({}, ~{} km)",
                    location.label,
                    location.source,
                    location.accuracy_meters / 1_000
                )
            } else if location.accuracy_meters > 0 {
                format!(
                    "{} ({}, ~{} m)",
                    location.label, location.source, location.accuracy_meters
                )
            } else {
                format!("{} ({})", location.label, location.source)
            }
        })
        .unwrap_or_else(|| "-".to_string())
}

struct MapCluster {
    client_ids: Vec<String>,
    title: String,
    detail: String,
    pos: egui::Pos2,
}

fn render_map_contents(
    ui: &mut egui::Ui,
    clients: &[ClientRow],
    selected_client_id: Option<&str>,
    client_filter: &mut String,
    selected_sink: &Arc<Mutex<Option<String>>>,
) {
    ui::panel(ui, |ui| {
        ui.horizontal(|ui| {
            ui::section_title(ui, "Client Map");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new("IP location is approximate")
                        .size(12.0)
                        .color(ui::COLOR_MUTED),
                );
            });
        });
        ui.add_space(8.0);
        ui.scope(|ui| {
            ui.spacing_mut().interact_size.y = ui::TOOLBAR_CONTROL_HEIGHT;
            ui.add_sized(
                [ui.available_width(), ui::TOOLBAR_CONTROL_HEIGHT],
                egui::TextEdit::singleline(client_filter)
                    .hint_text("Search by id, fingerprint, host, user, or OS")
                    .vertical_align(egui::Align::Center),
            );
        });
        ui.add_space(8.0);

        let located = clients
            .iter()
            .filter(|row| row.info.location.is_some())
            .count();
        if located == 0 {
            ui.add_space(18.0);
            ui.vertical_centered(|ui| {
                let (title, detail) = if clients.is_empty() {
                    (
                        "No matching clients",
                        "Clear or adjust the filter to show clients on the map.",
                    )
                } else {
                    (
                        "No geolocatable clients",
                        "GeoIP may be configured, but current clients have no public IP location. Local, LAN, VPN, proxy, and relay addresses cannot be placed on the map.",
                    )
                };
                ui.label(
                    egui::RichText::new(title)
                        .size(15.0)
                        .color(ui::COLOR_TEXT),
                );
                ui.label(
                    egui::RichText::new(detail)
                        .size(12.0)
                        .color(ui::COLOR_MUTED),
                );
                ui.label(
                    egui::RichText::new(
                        "If this is a public client, restart rdl-server with --geoip-db /path/GeoLite2-City.mmdb.",
                    )
                        .size(12.0)
                        .color(ui::COLOR_MUTED),
                );
            });
            ui.add_space(18.0);
            return;
        }

        ui.horizontal(|ui| {
            ui::metric(ui, "Located clients", located.to_string());
            ui.separator();
            ui::metric(ui, "Filtered clients", clients.len().to_string());
        });
        ui.add_space(8.0);

        let desired_size = egui::vec2(ui.available_width(), ui.available_height().max(380.0));
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
        let painter = ui.painter_at(rect);
        let map_rect = rect.shrink2(egui::vec2(10.0, 8.0));
        draw_world_map(&painter, map_rect);

        let clusters = map_clusters(clients, map_rect);
        for cluster in &clusters {
            let selected = cluster
                .client_ids
                .iter()
                .any(|id| selected_client_id == Some(id.as_str()));
            draw_map_cluster(&painter, cluster, selected);
        }

        if let Some(pointer) = response.hover_pos() {
            if let Some(cluster) = nearest_cluster(&clusters, pointer) {
                response
                    .clone()
                    .on_hover_text(format!("{}\n{}", cluster.title, cluster.detail));
            }
        }

        if response.clicked() {
            if let Some(pointer) = response.interact_pointer_pos() {
                if let Some(cluster) = nearest_cluster(&clusters, pointer) {
                    if let Some(client_id) = cluster.client_ids.first() {
                        if let Ok(mut target) = selected_sink.lock() {
                            *target = Some(client_id.clone());
                        }
                    }
                }
            }
        }
    });
}

fn filtered_clients(clients: &[ClientRow], filter: &str) -> Vec<ClientRow> {
    let filter = filter.trim().to_ascii_lowercase();
    clients
        .iter()
        .filter(|row| {
            if filter.is_empty() {
                return true;
            }
            row.info.id.to_ascii_lowercase().contains(&filter)
                || row.info.fingerprint.to_ascii_lowercase().contains(&filter)
                || row.info.hostname.to_ascii_lowercase().contains(&filter)
                || row.info.username.to_ascii_lowercase().contains(&filter)
                || row.info.os.to_ascii_lowercase().contains(&filter)
        })
        .cloned()
        .collect()
}

fn draw_world_map(painter: &egui::Painter, rect: egui::Rect) {
    painter.rect_filled(rect, 8.0, egui::Color32::from_rgb(232, 240, 248));
    painter.rect_stroke(
        rect,
        8.0,
        egui::Stroke::new(1.0, ui::COLOR_BORDER),
        egui::StrokeKind::Inside,
    );
    draw_graticule(painter, rect);
    draw_land_shapes(painter, rect);
}

fn draw_graticule(painter: &egui::Painter, rect: egui::Rect) {
    let stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(206, 218, 230));
    for lon in (-180..=180).step_by(60) {
        let x = map_project(rect, 0.0, lon as f64).x;
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            stroke,
        );
    }
    for lat in (-60..=60).step_by(30) {
        let y = map_project(rect, lat as f64, 0.0).y;
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            stroke,
        );
    }
}

fn draw_land_shapes(painter: &egui::Painter, rect: egui::Rect) {
    let fill = egui::Color32::from_rgb(214, 224, 213);
    let stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(174, 190, 174));
    for polygon in WORLD_LAND_POLYGONS {
        let points = polygon
            .iter()
            .map(|(lon, lat)| map_project(rect, *lat, *lon))
            .collect::<Vec<_>>();
        painter.add(egui::Shape::convex_polygon(points, fill, stroke));
    }
}

fn draw_map_cluster(painter: &egui::Painter, cluster: &MapCluster, selected: bool) {
    let count = cluster.client_ids.len();
    let radius = if count > 1 { 13.0 } else { 8.0 };
    let fill = if selected {
        ui::COLOR_ACCENT
    } else {
        ui::COLOR_GOOD
    };
    painter.circle_filled(cluster.pos, radius, fill);
    painter.circle_stroke(
        cluster.pos,
        radius,
        egui::Stroke::new(2.0, ui::COLOR_PANEL.gamma_multiply(0.95)),
    );
    if count > 1 {
        painter.text(
            cluster.pos,
            egui::Align2::CENTER_CENTER,
            count.to_string(),
            egui::FontId::proportional(11.0),
            ui::COLOR_PANEL,
        );
    }
}

fn map_clusters(clients: &[ClientRow], rect: egui::Rect) -> Vec<MapCluster> {
    let mut clusters = Vec::<MapCluster>::new();
    for row in clients {
        let Some(location) = row.info.location.as_ref() else {
            continue;
        };
        let lat = location.latitude().clamp(-90.0, 90.0);
        let lon = location.longitude().clamp(-180.0, 180.0);
        let pos = map_project(rect, lat, lon);
        let title = if row.info.hostname.trim().is_empty() {
            ui::compact_id(&row.info.id)
        } else {
            row.info.hostname.clone()
        };
        let detail = map_point_detail(row);

        if let Some(cluster) = clusters
            .iter_mut()
            .find(|cluster| cluster.pos.distance(pos) <= 18.0)
        {
            let count = cluster.client_ids.len() as f32;
            cluster.pos = egui::pos2(
                (cluster.pos.x * count + pos.x) / (count + 1.0),
                (cluster.pos.y * count + pos.y) / (count + 1.0),
            );
            cluster.client_ids.push(row.info.id.clone());
            cluster.title = format!("{} clients", cluster.client_ids.len());
            cluster.detail.push('\n');
            cluster.detail.push_str(&detail);
        } else {
            clusters.push(MapCluster {
                client_ids: vec![row.info.id.clone()],
                title,
                detail,
                pos,
            });
        }
    }
    clusters
}

fn nearest_cluster(clusters: &[MapCluster], pointer: egui::Pos2) -> Option<&MapCluster> {
    clusters
        .iter()
        .filter_map(|cluster| {
            let radius = if cluster.client_ids.len() > 1 {
                17.0
            } else {
                12.0
            };
            let distance = cluster.pos.distance(pointer);
            (distance <= radius).then_some((distance, cluster))
        })
        .min_by(|left, right| {
            left.0
                .partial_cmp(&right.0)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(_, cluster)| cluster)
}

fn map_point_detail(row: &ClientRow) -> String {
    let Some(location) = row.info.location.as_ref() else {
        return row.info.id.clone();
    };
    let accuracy = if location.accuracy_meters == 0 {
        "unknown accuracy".to_string()
    } else if location.accuracy_meters >= 1_000 {
        format!("~{} km", location.accuracy_meters / 1_000)
    } else {
        format!("~{} m", location.accuracy_meters)
    };
    format!(
        "{} / {} / {} / {} / {}",
        ui::compact_id(&row.info.id),
        row.info.username,
        row.info.os,
        location.label,
        accuracy
    )
}

fn map_project(rect: egui::Rect, latitude: f64, longitude: f64) -> egui::Pos2 {
    let x = rect.left() + (((longitude + 180.0) / 360.0) as f32 * rect.width());
    let y = rect.top() + (((90.0 - latitude) / 180.0) as f32 * rect.height());
    egui::pos2(x, y)
}

const WORLD_LAND_POLYGONS: &[&[(f64, f64)]] = &[
    &[
        (-168.0, 72.0),
        (-130.0, 72.0),
        (-100.0, 58.0),
        (-62.0, 52.0),
        (-58.0, 28.0),
        (-96.0, 16.0),
        (-118.0, 24.0),
        (-126.0, 48.0),
        (-168.0, 54.0),
    ],
    &[
        (-84.0, 13.0),
        (-50.0, 10.0),
        (-36.0, -16.0),
        (-52.0, -55.0),
        (-75.0, -47.0),
        (-81.0, -18.0),
    ],
    &[
        (-18.0, 36.0),
        (34.0, 36.0),
        (52.0, 8.0),
        (35.0, -35.0),
        (12.0, -35.0),
        (-16.0, 4.0),
    ],
    &[
        (-12.0, 72.0),
        (44.0, 70.0),
        (66.0, 54.0),
        (34.0, 36.0),
        (-10.0, 36.0),
        (-24.0, 56.0),
    ],
    &[
        (34.0, 36.0),
        (74.0, 55.0),
        (138.0, 52.0),
        (164.0, 62.0),
        (178.0, 42.0),
        (138.0, 8.0),
        (104.0, 2.0),
        (76.0, 8.0),
        (52.0, 24.0),
    ],
    &[
        (112.0, -10.0),
        (154.0, -12.0),
        (154.0, -38.0),
        (114.0, -44.0),
        (108.0, -26.0),
    ],
];
