use eframe::egui;
use serde::{Deserialize, Serialize};

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

#[derive(Deserialize, Serialize)]
struct Settings {
    stroke: egui::Stroke,
    ui_scale: f32,
    toggle_keybind: egui::KeyboardShortcut,
    clear_keybind: egui::KeyboardShortcut,
    toggle_fill_keybind: egui::KeyboardShortcut,
    toggle_menu_keybind: egui::KeyboardShortcut,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            stroke: egui::Stroke::new(2.0, egui::Color32::PURPLE),
            ui_scale: 1.5,
            toggle_keybind: egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::X),
            clear_keybind: egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::C),
            toggle_fill_keybind: egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::N),
            toggle_menu_keybind: egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::M),
        }
    }
}

impl Settings {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Ui Scale:");
            ui.add(egui::Slider::new(&mut self.ui_scale, 0.5..=3.0));

            if ui.button("Apply").clicked() {
                ui.ctx().set_pixels_per_point(self.ui_scale);
            }
        });

        ui.separator();

        ui.heading("Keybinds");

        ui.horizontal(|ui| {
            ui.label("Toggle:");
            ui.label(ui.ctx().format_shortcut(&self.toggle_keybind));
        });

        ui.horizontal(|ui| {
            ui.label("Clear:");
            ui.label(ui.ctx().format_shortcut(&self.clear_keybind));
        });

        ui.horizontal(|ui| {
            ui.label("Toggle Fill:");
            ui.label(ui.ctx().format_shortcut(&self.toggle_fill_keybind));
        });

        ui.horizontal(|ui| {
            ui.label("Toggle Menu:");
            ui.label(ui.ctx().format_shortcut(&self.toggle_menu_keybind));
        });
    }
}

struct Chameleos {
    menu_active: bool,
    settings_active: bool,

    lines: Vec<Vec<egui::Pos2>>,

    passthrough_active: bool,
    fill: bool,

    settings: Settings,
}

impl Chameleos {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings: Settings = cc
            .storage
            .and_then(|storage| eframe::get_value(storage, "settings"))
            .unwrap_or_default();

        cc.egui_ctx.set_pixels_per_point(settings.ui_scale);

        Self {
            menu_active: true,
            settings_active: false,

            lines: Vec::new(),

            passthrough_active: false,
            fill: true,

            settings,
        }
    }
}

impl Chameleos {
    fn clear(&mut self) {
        self.lines.clear();
    }
}

impl eframe::App for Chameleos {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.menu_active {
            egui::TopBottomPanel::top("menu-bar").show(ctx, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    // NOTE: ideally we'd use egui::containers::Sides
                    // but that causes borrowing issues

                    ui.menu_button("File", |ui| {
                        if ui.button("Settings").clicked() {
                            self.settings_active = true;

                            // close menu
                            // should happen by itself but not necessarily if we change the
                            // general PopupCloseBehavior
                            ui.close();
                        }

                        if ui.button("Exit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });

                    if ui.button("Toggle Fill").clicked() {
                        self.fill = !self.fill;
                    }

                    if ui.button("Hide Menu").clicked() {
                        self.menu_active = false;
                    }

                    ui.with_layout(
                        egui::Layout::right_to_left(ui.layout().vertical_align()),
                        |ui| {
                            // unfortunately, putting this in a submenu doesn't seem to work :/
                            ui.add(&mut self.settings.stroke);

                            ui.separator();

                            if ui.button("Clear Paint").clicked() {
                                self.clear();
                            }
                        },
                    );
                });
            });
        }

        egui::Window::new("Settings")
            .open(&mut self.settings_active)
            .show(ctx, |ui| {
                self.settings.ui(ui);
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                if ui.input_mut(|i| i.consume_shortcut(&self.settings.toggle_keybind)) {
                    if self.passthrough_active {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Title(
                            "chameleos".to_string(),
                        ));
                    } else {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Title(
                            "chameleos-passthrough".to_string(),
                        ));
                    }

                    self.passthrough_active = !self.passthrough_active;
                }

                if ui.input_mut(|i| i.consume_shortcut(&self.settings.clear_keybind)) {
                    self.clear();
                }

                if ui.input_mut(|i| i.consume_shortcut(&self.settings.toggle_fill_keybind)) {
                    self.fill = !self.fill;
                }

                if ui.input_mut(|i| i.consume_shortcut(&self.settings.toggle_menu_keybind)) {
                    self.menu_active = !self.menu_active;
                }

                // mostly taken from egui's painting example
                // https://github.com/emilk/egui/blob/6ac155c5cd3ee9d194579edc964c5659dfe70ab0/crates/egui_demo_lib/src/demo/painting.rs

                let (mut response, painter) =
                    ui.allocate_painter(ui.available_size_before_wrap(), egui::Sense::drag());

                if response.hovered() {
                    ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
                } else {
                    ctx.set_cursor_icon(egui::CursorIcon::Default);
                }

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
                            egui::Shape::line(line.clone(), self.settings.stroke)
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

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "settings", &self.settings);
    }
}
