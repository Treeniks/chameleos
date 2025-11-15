use std::collections::HashMap;

use wayland_backend::client::ObjectId;

use wayland_client::Connection;
use wayland_client::Dispatch;
use wayland_client::Proxy;
use wayland_client::QueueHandle;
use wayland_client::WEnum;

use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::Shape;
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::WpCursorShapeDeviceV1;

use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_pad_v2::ZwpTabletPadV2;
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_seat_v2::ZwpTabletSeatV2;
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2::ZwpTabletToolV2;
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_v2::ZwpTabletV2;

use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_seat_v2::EVT_PAD_ADDED_OPCODE;
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_seat_v2::EVT_TABLET_ADDED_OPCODE;
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_seat_v2::EVT_TOOL_ADDED_OPCODE;

use log::Level;
use log::log;

#[derive(Default)]
pub struct TabletState {
    event_sequence: EventSequence,

    tablet_seat: Option<ZwpTabletSeatV2>,
    tablet_cursor_shape_devices: HashMap<ObjectId, WpCursorShapeDeviceV1>,

    pos: Option<(f64, f64)>,
    pen_is_down: bool,
    button_held: bool,
}

impl TabletState {
    pub fn set_tablet_seat(&mut self, tablet_seat: ZwpTabletSeatV2) {
        self.tablet_seat = Some(tablet_seat);
    }

    fn update_state(&mut self, sequence: EventSequence) {
        if let Some(new_pos) = sequence.motion {
            self.pos = Some(new_pos);
        }

        if sequence.pen_down {
            self.pen_is_down = true;
        }
        if sequence.pen_up {
            self.pen_is_down = false;
        }

        if sequence.button_pressed {
            self.button_held = true;
        }
        if sequence.button_released {
            self.button_held = false;
        }
    }
}

impl Dispatch<ZwpTabletSeatV2, (), super::State> for TabletState {
    fn event(
        state: &mut super::State,
        _tablet_seat: &ZwpTabletSeatV2,
        event: <ZwpTabletSeatV2 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qhandle: &QueueHandle<super::State>,
    ) {
        log!(target: "chameleos::wayland", Level::Info, "ZwpTabletSeatV2: {:?}", event);

        use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_seat_v2::Event;
        match event {
            Event::TabletAdded { id: _ } => {}
            Event::ToolAdded { id } => {
                let cursor_shape_device =
                    state
                        .wayland
                        .cursor_shape_manager
                        .get_tablet_tool_v2(&id, qhandle, ());
                state
                    .tablet
                    .tablet_cursor_shape_devices
                    .insert(id.id(), cursor_shape_device);
            }
            Event::PadAdded { id: _ } => {}
            _ => {}
        }
    }

    wayland_client::event_created_child!(super::State, ZwpTabletSeatV2, [
        EVT_TABLET_ADDED_OPCODE => (ZwpTabletV2, ()),
        EVT_TOOL_ADDED_OPCODE => (ZwpTabletToolV2, ()),
        EVT_PAD_ADDED_OPCODE => (ZwpTabletPadV2, ()),
    ]);
}

impl Dispatch<ZwpTabletToolV2, (), super::State> for TabletState {
    fn event(
        state: &mut super::State,
        tablet_tool: &ZwpTabletToolV2,
        event: <ZwpTabletToolV2 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<super::State>,
    ) {
        log!(target: "chameleos::wayland", Level::Debug, "ZwpTabletToolV2: {:?}", event);

        let tablet = &mut state.tablet;
        let draw = &mut state.draw;

        // TODO this is basically identical to MouseState
        if let Some(sequence) = tablet.event_sequence.dispatch(event) {
            tablet.update_state(sequence);

            if let Some(device) = tablet.tablet_cursor_shape_devices.get(&tablet_tool.id())
                && let Some(serial) = sequence.enter_serial
            {
                device.set_shape(serial, Shape::Crosshair);
            }

            let draw_pos = if tablet.pen_is_down {
                sequence.motion
            } else if sequence.pen_down {
                tablet.pos
            } else {
                None
            };

            if let Some(pos) = draw_pos {
                draw.add_point_to_line(pos);
            }

            let erase_pos = if tablet.button_held {
                sequence.motion
            } else if sequence.button_pressed {
                tablet.pos
            } else {
                None
            };

            if let Some(pos) = erase_pos {
                draw.erase(pos);
            }

            if sequence.pen_up {
                draw.cut_line();
            }
        }
    }
}

#[derive(Default, Clone, Copy)]
struct EventSequence {
    motion: Option<(f64, f64)>,

    pen_down: bool,
    pen_up: bool,

    button_pressed: bool,
    button_released: bool,

    enter_serial: Option<u32>,
}

impl EventSequence {
    fn dispatch(&mut self, event: <ZwpTabletToolV2 as Proxy>::Event) -> Option<Self> {
        use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2::Event;
        match event {
            Event::ProximityIn {
                serial,
                tablet: _,
                surface: _,
            } => {
                self.enter_serial = Some(serial);
                None
            }
            Event::Down { serial: _ } => {
                self.pen_down = true;
                None
            }
            Event::Up => {
                self.pen_up = true;
                None
            }
            Event::Motion { x, y } => {
                self.motion = Some((x, y));
                None
            }
            Event::Button {
                serial: _,
                button,
                state: button_state,
            } => {
                use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2::ButtonState;
                if button == 331 {
                    match button_state {
                        WEnum::Value(ButtonState::Released) => self.button_released = true,
                        WEnum::Value(ButtonState::Pressed) => self.button_pressed = true,
                        _ => {}
                    }
                }
                None
            }
            Event::Frame { time: _ } => {
                let mut tmp = Self::default();
                std::mem::swap(self, &mut tmp);
                Some(tmp)
            }
            Event::Pressure { pressure: _ } => {
                // TODO maybe support pressure sensitivity down the line
                None
            }
            _ => None,
        }
    }
}
