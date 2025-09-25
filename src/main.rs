use eframe::egui;
use serde::{Deserialize, Serialize};

mod keybind;
use keybind::Keybind;

fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id("chameleos")
            .with_always_on_top()
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

    fill_on_startup: bool,
    fill_color: egui::Rgba,

    toggle_keybind: Keybind,
    clear_keybind: Keybind,
    undo_keybind: Keybind,
    toggle_fill_keybind: Keybind,
    toggle_menu_keybind: Keybind,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            stroke: egui::Stroke::new(2.0, egui::Color32::PURPLE),
            ui_scale: 1.5,
            fill_on_startup: true,
            fill_color: egui::Rgba::from_rgb(1.0, 0.0, 1.0),

            toggle_keybind: Keybind::new(egui::KeyboardShortcut::new(
                egui::Modifiers::NONE,
                egui::Key::X,
            )),
            clear_keybind: Keybind::new(egui::KeyboardShortcut::new(
                egui::Modifiers::NONE,
                egui::Key::C,
            )),
            undo_keybind: Keybind::new(egui::KeyboardShortcut::new(
                egui::Modifiers::NONE,
                egui::Key::U,
            )),
            toggle_fill_keybind: Keybind::new(egui::KeyboardShortcut::new(
                egui::Modifiers::NONE,
                egui::Key::N,
            )),
            toggle_menu_keybind: Keybind::new(egui::KeyboardShortcut::new(
                egui::Modifiers::NONE,
                egui::Key::M,
            )),
        }
    }
}

impl Settings {
    fn clear_expecting(&mut self) {
        self.toggle_keybind.clear_expecting();
        self.clear_keybind.clear_expecting();
        self.undo_keybind.clear_expecting();
        self.toggle_fill_keybind.clear_expecting();
        self.toggle_menu_keybind.clear_expecting();
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Ui Scale:");
            ui.add(egui::Slider::new(&mut self.ui_scale, 0.5..=3.0));

            if ui.button("Apply").clicked() {
                ui.ctx().set_pixels_per_point(self.ui_scale);
            }
        });

        ui.separator();

        ui.checkbox(&mut self.fill_on_startup, "Fill On Startup");

        ui.horizontal(|ui| {
            ui.label("Fill Color:");
            egui::widgets::color_picker::color_edit_button_rgba(
                ui,
                &mut self.fill_color,
                egui::widgets::color_picker::Alpha::Opaque,
            );
        });

        ui.separator();

        ui.heading("Keybinds");

        egui::Grid::new("keybinds-grid")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("Toggle:");
                ui.add(&mut self.toggle_keybind);
                ui.end_row();

                ui.label("Clear:");
                ui.add(&mut self.clear_keybind);
                ui.end_row();

                ui.label("Undo:");
                ui.add(&mut self.undo_keybind);
                ui.end_row();

                ui.label("Toggle Fill:");
                ui.add(&mut self.toggle_fill_keybind);
                ui.end_row();

                ui.label("Toggle Menu:");
                ui.add(&mut self.toggle_menu_keybind);
                ui.end_row();
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
            fill: settings.fill_on_startup,

            settings,
        }
    }
}

impl Chameleos {
    fn clear(&mut self) {
        self.lines.clear();
    }

    fn undo(&mut self) {
        if !self.lines.is_empty() {
            self.lines.pop();
            if !self.lines.is_empty() {
                self.lines.pop();
            }
        }
    }
}

impl eframe::App for Chameleos {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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

                    let toggle_fill_button = egui::Button::new("Toggle Fill").shortcut_text(
                        ctx.format_shortcut(self.settings.toggle_fill_keybind.shortcut()),
                    );
                    if ui.add(toggle_fill_button).clicked() {
                        self.fill = !self.fill;
                    }

                    let hide_menu_button = egui::Button::new("Hide Menu").shortcut_text(
                        ctx.format_shortcut(self.settings.toggle_menu_keybind.shortcut()),
                    );
                    if ui.add(hide_menu_button).clicked() {
                        self.menu_active = false;
                    }

                    ui.with_layout(
                        egui::Layout::right_to_left(ui.layout().vertical_align()),
                        |ui| {
                            // unfortunately, putting this in a submenu doesn't seem to work :/
                            ui.add(&mut self.settings.stroke);

                            ui.separator();

                            let clear_button = egui::Button::new("Clear").shortcut_text(
                                ctx.format_shortcut(self.settings.clear_keybind.shortcut()),
                            );
                            if ui.add(clear_button).clicked() {
                                self.clear();
                            }

                            let undo_button = egui::Button::new("Undo").shortcut_text(
                                ctx.format_shortcut(self.settings.undo_keybind.shortcut()),
                            );
                            if ui.add(undo_button).clicked() {
                                self.undo();
                            }
                        },
                    );
                });
            });
        }

        if egui::Window::new("Settings")
            .open(&mut self.settings_active)
            .show(ctx, |ui| {
                self.settings.ui(ui);
            })
            .is_none()
        {
            // if the window is not open, make sure none of the keybinds are recording right now
            self.settings.clear_expecting();
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                if ui.input_mut(|i| i.consume_shortcut(self.settings.toggle_keybind.shortcut())) {
                    if self.passthrough_active {
                        ctx.send_viewport_cmd(egui::ViewportCommand::MousePassthrough(false));
                    } else {
                        ctx.send_viewport_cmd(egui::ViewportCommand::MousePassthrough(true));
                    }

                    self.passthrough_active = !self.passthrough_active;
                }

                if ui.input_mut(|i| i.consume_shortcut(self.settings.clear_keybind.shortcut())) {
                    self.clear();
                }

                if ui.input_mut(|i| i.consume_shortcut(self.settings.undo_keybind.shortcut())) {
                    self.undo();
                }

                if ui
                    .input_mut(|i| i.consume_shortcut(self.settings.toggle_fill_keybind.shortcut()))
                {
                    self.fill = !self.fill;
                }

                if ui
                    .input_mut(|i| i.consume_shortcut(self.settings.toggle_menu_keybind.shortcut()))
                {
                    self.menu_active = !self.menu_active;
                }

                // mostly taken from egui's painting example
                // https://github.com/emilk/egui/blob/6ac155c5cd3ee9d194579edc964c5659dfe70ab0/crates/egui_demo_lib/src/demo/painting.rs

                let (response, painter) =
                    ui.allocate_painter(ui.available_size_before_wrap(), egui::Sense::drag());

                let mut response = response.on_hover_cursor(egui::CursorIcon::Crosshair);

                if self.lines.is_empty() {
                    self.lines.push(vec![]);
                }

                let current_line = self.lines.last_mut().unwrap();

                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    if current_line.last() != Some(&pointer_pos) {
                        current_line.push(pointer_pos);
                        response.mark_changed();
                    }
                } else if !current_line.is_empty() {
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
                            egui::Shape::circle_filled(
                                line[0],
                                self.settings.stroke.width * 0.50,
                                self.settings.stroke.color,
                            )
                        }
                    });

                painter.extend(shapes);
            });
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        if self.fill {
            self.settings.fill_color.to_array()
        } else {
            [0.0, 0.0, 0.0, 0.0]
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "settings", &self.settings);
    }
}
