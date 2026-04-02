use crate::core::SharedSettings;
use eframe::egui;

pub struct ZonePreview {
    id: String,
    is_left: bool,
}

impl ZonePreview {
    pub fn new(id: &str, is_left: bool) -> Self {
        Self {
            id: id.to_string(),
            is_left,
        }
    }

    pub fn show(&self, ctx: &egui::Context, settings: SharedSettings) {
        let s = settings.read();
        let show_preview = s.show_zone_preview;
        let zone_width = s.zone_width;
        drop(s);

        let viewport_id = egui::ViewportId::from_hash_of(&self.id);

        if show_preview {
            let zone_height = 1440.0;
            let pos = if self.is_left {
                [0.0, 0.0]
            } else {
                [2560.0 - zone_width, 0.0]
            };

            ctx.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd_to(
                viewport_id,
                egui::ViewportCommand::InnerSize(egui::Vec2::new(zone_width, zone_height)),
            );
            ctx.send_viewport_cmd_to(
                viewport_id,
                egui::ViewportCommand::OuterPosition(egui::Pos2::new(pos[0], pos[1])),
            );

            ctx.show_viewport_immediate(
                viewport_id,
                egui::ViewportBuilder::default()
                    .with_title("Zone Preview")
                    .with_decorations(false)
                    .with_transparent(true)
                    .with_always_on_top()
                    .with_taskbar(false)
                    .with_inner_size([zone_width, zone_height])
                    .with_position(pos),
                move |ctx, _| {
                    ctx.request_repaint();
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE.fill(egui::Color32::TRANSPARENT))
                        .show_inside(ctx, |ui| {
                            let rect = ui.max_rect();
                            ui.painter().rect_stroke(
                                rect,
                                0.0,
                                egui::Stroke::new(2.0, egui::Color32::RED),
                                egui::StrokeKind::Outside,
                            );
                        });
                },
            );
        } else {
            ctx.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Visible(false));
        }
    }
}
