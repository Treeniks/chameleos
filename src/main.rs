use eframe::egui;

fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id("chameleos")
            // unfortunately this doesn't work with hyprland :/ (yet?)
            // .with_mouse_passthrough(true)
            // .with_always_on_top()
            .with_transparent(true),

        ..Default::default()
    };
    eframe::run_native(
        "chameleos",
        native_options,
        Box::new(|cc| Ok(Box::new(Chameleos::new(cc)))),
    )
}

struct Chameleos {
    menu_active: bool,

    lines: Vec<Vec<egui::Pos2>>,
    stroke: egui::Stroke,

    fill: bool,
}

impl Default for Chameleos {
    fn default() -> Self {
        Self {
            menu_active: true,

            lines: Vec::new(),
            stroke: egui::Stroke::new(2.0, egui::Color32::PURPLE),

            fill: true,
        }
    }
}

impl Chameleos {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_pixels_per_point(2.0);
        Self::default()
    }
}

impl eframe::App for Chameleos {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.menu_active {
            egui::TopBottomPanel::top("menu-bar").show(ctx, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.menu_button("Paint", |ui| {
                        if ui.button("Clear").clicked() {
                            self.lines.clear();
                        }
                    });

                    if ui.button("Toggle Fill").clicked() {
                        self.fill = !self.fill;
                    }

                    if ui.button("Hide Menu").clicked() {
                        self.menu_active = false;
                    }
                });
            });
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                if ui.input(|i| i.key_pressed(egui::Key::X)) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Title(
                        "chameleos-passthrough".to_string(),
                    ));
                }

                if ui.input(|i| i.key_pressed(egui::Key::C)) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Title("chameleos".to_string()));
                }

                // mostly taken from egui's painting example
                // https://github.com/emilk/egui/blob/6ac155c5cd3ee9d194579edc964c5659dfe70ab0/crates/egui_demo_lib/src/demo/painting.rs

                let (mut response, painter) =
                    ui.allocate_painter(ui.available_size_before_wrap(), egui::Sense::drag());

                if self.lines.is_empty() {
                    self.lines.push(vec![]);
                }

                let current_line = self.lines.last_mut().unwrap();

                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    if current_line.last() != Some(&pointer_pos) {
                        current_line.push(pointer_pos);
                        response.mark_changed();
                    }
                } else {
                    self.lines.push(vec![]);
                    response.mark_changed();
                }

                let shapes = self
                    .lines
                    .iter()
                    .filter(|line| !line.is_empty())
                    .map(|line| {
                        if line.len() >= 2 {
                            egui::Shape::line(line.clone(), self.stroke)
                        } else {
                            egui::Shape::circle_filled(line[0], 2.0, egui::Color32::PURPLE)
                        }
                    });

                painter.extend(shapes);
            });
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        if self.fill {
            [1.0, 0.0, 1.0, 1.0]
        } else {
            [0.0, 0.0, 0.0, 0.0]
        }
    }
}
