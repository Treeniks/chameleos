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

#[derive(Default)]
struct Chameleos {}

impl Chameleos {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }
}

impl eframe::App for Chameleos {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
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

                ui.heading("Hello World!");
            });
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }
}
