use eframe::egui;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug)]
#[derive(Deserialize, Serialize)]
pub struct Keybind {
    shortcut: egui::KeyboardShortcut,

    #[serde(skip)]
    #[serde(default)]
    expecting: bool,
}

impl Keybind {
    pub fn new(shortcut: egui::KeyboardShortcut) -> Self {
        Self {
            shortcut,
            expecting: false,
        }
    }

    pub fn shortcut(&self) -> egui::KeyboardShortcut {
        self.shortcut
    }
}

impl egui::Widget for &mut Keybind {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let response = ui.button(ui.ctx().format_shortcut(&self.shortcut()));

        if response.clicked() {
            self.expecting = true;
        }

        if self.expecting {
            if let Some((key, modifiers)) = ui.input(|i| {
                i.events.iter().find_map(|event| match event {
                    egui::Event::Key {
                        key,
                        physical_key: _,
                        pressed: _,
                        repeat: _,
                        modifiers,
                    } => Some((*key, *modifiers)),
                    _ => None,
                })
            }) {
                self.shortcut = egui::KeyboardShortcut::new(modifiers, key);
                self.expecting = false;

                // consume this newly created shortcut so it doesn't immediately activate
                ui.input_mut(|i| i.consume_shortcut(&self.shortcut));
            }
        }

        response
    }
}
