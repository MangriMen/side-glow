use crate::core::SharedSettings;
use eframe::egui;

pub struct GlowPanel {
    id: String,
    title: String,
    is_left: bool,
    pos: [f32; 2],
}

impl GlowPanel {
    pub fn new(id: &str, title: &str, is_left: bool, pos: [f32; 2]) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            is_left,
            pos,
        }
    }

    pub fn show(
        &self,
        ctx: &egui::Context,
        settings: SharedSettings,
        monitor_w: f32,
        monitor_h: f32,
    ) {
        let settings = settings.clone();
        let is_left = self.is_left;
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of(&self.id),
            egui::ViewportBuilder::default()
                .with_title(&self.title)
                .with_decorations(false)
                .with_transparent(false)
                .with_always_on_top()
                .with_taskbar(false)
                .with_position(self.pos)
                .with_inner_size([monitor_w, monitor_h]),
            move |ctx, _| {
                ctx.request_repaint();
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.fill(egui::Color32::BLACK))
                    .show_inside(ctx, |ui| {
                        let s = settings.read();
                        let color_arr = if is_left { s.left_color } else { s.right_color };
                        let color = egui::Color32::from_rgb(
                            (color_arr[0] * 255.0) as u8,
                            (color_arr[1] * 255.0) as u8,
                            (color_arr[2] * 255.0) as u8,
                        );
                        Self::draw_gradient(ui, color, s.glow_depth, is_left);
                    });
            },
        );
    }

    fn draw_gradient(ui: &mut egui::Ui, color: egui::Color32, depth: f32, is_left: bool) {
        let rect = ui.max_rect();
        let gradient_width = rect.width() * depth;
        let mut mesh = egui::Mesh::default();
        let transparent = egui::Color32::TRANSPARENT;

        if is_left {
            mesh.colored_vertex(
                egui::pos2(rect.right() - gradient_width, rect.top()),
                transparent,
            );
            mesh.colored_vertex(egui::pos2(rect.right(), rect.top()), color);
            mesh.colored_vertex(egui::pos2(rect.right(), rect.bottom()), color);
            mesh.colored_vertex(
                egui::pos2(rect.right() - gradient_width, rect.bottom()),
                transparent,
            );
        } else {
            mesh.colored_vertex(rect.min, color);
            mesh.colored_vertex(
                egui::pos2(rect.left() + gradient_width, rect.top()),
                transparent,
            );
            mesh.colored_vertex(
                egui::pos2(rect.left() + gradient_width, rect.bottom()),
                transparent,
            );
            mesh.colored_vertex(egui::pos2(rect.left(), rect.bottom()), color);
        }
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        ui.painter().add(egui::Shape::mesh(mesh));
    }
}
