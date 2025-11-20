use wayland_client::Connection;
use wayland_client::Dispatch;
use wayland_client::Proxy;
use wayland_client::QueueHandle;
use wayland_client::WEnum;

use wayland_client::protocol::wl_pointer::WlPointer;

use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::Shape;
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::WpCursorShapeDeviceV1;

use log::Level;
use log::log;

#[inline(always)]
pub fn draw_pos(
    pressed: bool,
    motion: Option<(f64, f64)>,
    held: bool,
    pos: Option<(f64, f64)>,
) -> Option<(f64, f64)> {
    held.then_some(motion)
        .flatten()
        .or_else(|| pressed.then_some(pos).flatten())
}

#[derive(Default)]
pub struct MouseState {
    event_sequence: EventSequence,

    cursor_shape_device: Option<WpCursorShapeDeviceV1>,

    mouse_pos: Option<(f64, f64)>,
    left_button_held: bool,
    right_button_held: bool,
}

impl MouseState {
    pub fn set_cursor_shape_device(&mut self, cursor_shape_device: WpCursorShapeDeviceV1) {
        self.cursor_shape_device = Some(cursor_shape_device)
    }

    fn update_state(&mut self, sequence: EventSequence) {
        if let Some(new_pos) = sequence.motion {
            self.mouse_pos = Some(new_pos);
        }

        if let Some(_) = sequence.leave_serial {
            self.mouse_pos = None;
        }

        if sequence.left_button_pressed {
            self.left_button_held = true;
        }
        if sequence.left_button_released {
            self.left_button_held = false;
        }

        if sequence.right_button_pressed {
            self.right_button_held = true;
        }
        if sequence.right_button_released {
            self.right_button_held = false;
        }
    }
}

impl Dispatch<WlPointer, (), super::State> for MouseState {
    fn event(
        state: &mut super::State,
        _pointer: &WlPointer,
        event: <WlPointer as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<super::State>,
    ) {
        log!(target: "chameleos::wayland", Level::Debug, "WlPointer: {:?}", event);

        let mouse = &mut state.mouse;
        let draw = &mut state.draw;

        if let Some(sequence) = mouse.event_sequence.dispatch(event) {
            mouse.update_state(sequence);

            if let Some(ref device) = mouse.cursor_shape_device
                && let Some(serial) = sequence.enter_serial
            {
                device.set_shape(serial, Shape::Crosshair);
            }

            let pen_pos = draw_pos(
                sequence.left_button_pressed,
                sequence.motion,
                mouse.left_button_held,
                mouse.mouse_pos,
            );

            if let Some(pos) = pen_pos {
                draw.add_point_to_line(pos);
            }

            let erase_pos = draw_pos(
                sequence.right_button_pressed,
                sequence.motion,
                mouse.right_button_held,
                mouse.mouse_pos,
            );

            if let Some(pos) = erase_pos {
                draw.erase(pos);
            }

            if sequence.left_button_released {
                draw.cut_line();
            }
        }
    }
}

#[derive(Default, Clone, Copy)]
struct EventSequence {
    motion: Option<(f64, f64)>,

    left_button_pressed: bool,
    left_button_released: bool,
    right_button_pressed: bool,
    right_button_released: bool,

    enter_serial: Option<u32>,
    leave_serial: Option<u32>,
}

impl EventSequence {
    fn dispatch(&mut self, event: <WlPointer as Proxy>::Event) -> Option<Self> {
        use wayland_client::protocol::wl_pointer::Event;
        match event {
            Event::Enter {
                serial,
                surface: _,
                surface_x,
                surface_y,
            } => {
                self.enter_serial = Some(serial);
                self.motion = Some((surface_x, surface_y));
                None
            }
            Event::Leave { serial, surface: _ } => {
                self.leave_serial = Some(serial);
                None
            }
            Event::Motion {
                time: _,
                surface_x,
                surface_y,
            } => {
                self.motion = Some((surface_x, surface_y));
                None
            }
            Event::Button {
                serial: _,
                time: _,
                button,
                state: button_state,
            } => {
                use wayland_client::protocol::wl_pointer::ButtonState;

                // left mouse button
                if button == 272 {
                    match button_state {
                        WEnum::Value(ButtonState::Released) => self.left_button_released = true,
                        WEnum::Value(ButtonState::Pressed) => self.left_button_pressed = true,
                        _ => {}
                    }
                }

                if button == 273 {
                    match button_state {
                        WEnum::Value(ButtonState::Released) => self.right_button_released = true,
                        WEnum::Value(ButtonState::Pressed) => self.right_button_pressed = true,
                        _ => {}
                    }
                }

                None
            }
            Event::Frame => {
                let mut tmp = Self::default();
                std::mem::swap(self, &mut tmp);
                Some(tmp)
            }
            _ => None,
        }
    }
}
