use std::collections::HashMap;

use wayland_client::Connection;
use wayland_client::Dispatch;
use wayland_client::Proxy;
use wayland_client::QueueHandle;

use wayland_client::protocol::wl_touch::WlTouch;

use log::Level;
use log::log;

use super::draw::DrawState;

#[derive(Default)]
pub struct TouchState {
    /// Events since last `frame` (must be applied in order).
    batch: Vec<TouchEvt>,
    /// Active touch points and last surface-local positions.
    slots: HashMap<i32, (f64, f64)>,
    /// Touch id that owns the current stroke (first finger down in a gesture).
    drawing_id: Option<i32>,
}

#[derive(Clone, Copy)]
enum TouchEvt {
    Down { id: i32, x: f64, y: f64 },
    Up { id: i32 },
    Motion { id: i32, x: f64, y: f64 },
}

impl TouchState {
    fn cancel_all(&mut self, draw: &mut DrawState) {
        self.batch.clear();
        self.slots.clear();
        if self.drawing_id.take().is_some() {
            draw.cut_line();
        }
        draw.set_touch_drawing_cursor_surface(None);
    }

    /// Clear queued touch state and end any in-progress stroke (overlay input turned off).
    pub fn reset_for_deactivate(&mut self, draw: &mut DrawState) {
        self.cancel_all(draw);
    }

    fn flush_frame(&mut self, draw: &mut DrawState) {
        for ev in self.batch.drain(..) {
            match ev {
                TouchEvt::Down { id, x, y } => {
                    self.slots.insert(id, (x, y));
                    if self.drawing_id.is_none() && self.slots.len() == 1 {
                        self.drawing_id = Some(id);
                        draw.add_point_to_line_touch((x, y));
                        draw.set_touch_drawing_cursor_surface(Some((x, y)));
                    }
                }
                TouchEvt::Motion { id, x, y } => {
                    self.slots.insert(id, (x, y));
                    if Some(id) == self.drawing_id {
                        draw.add_point_to_line_touch((x, y));
                        draw.set_touch_drawing_cursor_surface(Some((x, y)));
                    }
                }
                TouchEvt::Up { id } => {
                    self.slots.remove(&id);
                    if Some(id) == self.drawing_id {
                        draw.cut_line();
                        self.drawing_id = None;
                        draw.set_touch_drawing_cursor_surface(None);
                    }
                }
            }
        }
    }
}

impl Dispatch<WlTouch, (), super::State> for TouchState {
    fn event(
        state: &mut super::State,
        _touch: &WlTouch,
        event: <WlTouch as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<super::State>,
    ) {
        log!(target: "chameleos::wayland", Level::Debug, "WlTouch: {:?}", event);

        use wayland_client::protocol::wl_touch::Event;
        if !state.active {
            if matches!(event, Event::Cancel) {
                state.touch.cancel_all(&mut state.draw);
            }
            return;
        }

        let touch = &mut state.touch;
        let draw = &mut state.draw;

        match event {
            Event::Down {
                id,
                x,
                y,
                surface: _,
                serial: _,
                time: _,
            } => {
                touch.batch.push(TouchEvt::Down { id, x, y });
            }
            Event::Up {
                id,
                serial: _,
                time: _,
            } => {
                touch.batch.push(TouchEvt::Up { id });
            }
            Event::Motion { id, x, y, time: _ } => {
                touch.batch.push(TouchEvt::Motion { id, x, y });
            }
            Event::Frame => {
                touch.flush_frame(draw);
            }
            Event::Cancel => {
                touch.cancel_all(draw);
            }
            _ => {}
        }
    }
}
