use eframe::egui;

fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions::default();
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
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Hello World!");
        });
    }
}
