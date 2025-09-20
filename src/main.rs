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
        "Chameleos",
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
                ui.heading("Hello World!");
            });
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }
}
