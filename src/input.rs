// SPDX-License-Identifier: GPL-3.0-or-later

pub mod libinput;

use std::collections::HashMap;

use crate::{
    api::msg::{CallbackId, Modifier, ModifierMask, MouseEdge, OutgoingMsg},
    focus::FocusTarget,
    state::WithState,
    window::WindowElement,
};
use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, InputBackend, InputEvent,
        KeyState, KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
    },
    desktop::{layer_map_for_output, space::SpaceElement},
    input::{
        keyboard::{keysyms, FilterResult},
        pointer::{AxisFrame, ButtonEvent, MotionEvent, RelativeMotionEvent},
    },
    reexports::input::{self, Led},
    utils::{Logical, Point, SERIAL_COUNTER},
    wayland::{seat::WaylandFocus, shell::wlr_layer},
};
use xkbcommon::xkb::Keysym;

use crate::state::State;

use self::libinput::LibinputSetting;

#[derive(Default, Debug)]
pub struct InputState {
    /// A hashmap of modifier keys and keycodes to callback IDs
    pub keybinds: HashMap<(ModifierMask, Keysym), CallbackId>,
    /// A hashmap of modifier keys and mouse button codes to callback IDs
    pub mousebinds: HashMap<(ModifierMask, u32, MouseEdge), CallbackId>,
    pub reload_keybind: Option<(ModifierMask, Keysym)>,
    pub kill_keybind: Option<(ModifierMask, Keysym)>,
    /// User defined libinput settings that will be applied
    pub libinput_settings: Vec<LibinputSetting>,
    /// All libinput devices that have been connected
    pub libinput_devices: Vec<input::Device>,
}

impl InputState {
    pub fn new() -> Self {
        Default::default()
    }
}

#[derive(Debug)]
enum KeyAction {
    /// Call a callback from a config process
    CallCallback(CallbackId),
    Quit,
    SwitchVt(i32),
    ReloadConfig,
}

impl State {
    pub fn process_input_event<B: InputBackend>(&mut self, event: InputEvent<B>) {
        match event {
            // TODO: rest of input events

            // InputEvent::DeviceAdded { device } => todo!(),
            // InputEvent::DeviceRemoved { device } => todo!(),
            InputEvent::Keyboard { event } => self.keyboard::<B>(event),
            InputEvent::PointerMotion { event } => self.pointer_motion::<B>(event),
            InputEvent::PointerMotionAbsolute { event } => self.pointer_motion_absolute::<B>(event),
            InputEvent::PointerButton { event } => self.pointer_button::<B>(event),
            InputEvent::PointerAxis { event } => self.pointer_axis::<B>(event),

            _ => (),
        }
    }

    /// Get the [`FocusTarget`] under `point`.
    pub fn surface_under<P>(&self, point: P) -> Option<(FocusTarget, Point<i32, Logical>)>
    where
        P: Into<Point<f64, Logical>>,
    {
        let point: Point<f64, Logical> = point.into();

        let output = self.space.outputs().find(|op| {
            self.space
                .output_geometry(op)
                .expect("called output_geometry on unmapped output (this shouldn't happen here)")
                .contains(point.to_i32_round())
        })?;

        let output_geo = self
            .space
            .output_geometry(output)
            .expect("called output_geometry on unmapped output");

        let layers = layer_map_for_output(output);

        let top_fullscreen_window = self.focus_state.focus_stack.iter().rev().find(|win| {
            win.with_state(|state| {
                state.fullscreen_or_maximized.is_fullscreen()
                    && state.tags.iter().any(|tag| tag.active())
            })
        });

        if let Some(window) = top_fullscreen_window {
            Some((FocusTarget::from(window.clone()), output_geo.loc))
        } else if let (Some(layer), _) | (None, Some(layer)) = (
            layers.layer_under(wlr_layer::Layer::Overlay, point),
            layers.layer_under(wlr_layer::Layer::Top, point),
        ) {
            let layer_loc = layers.layer_geometry(layer).expect("no layer geo").loc;
            Some((FocusTarget::from(layer.clone()), output_geo.loc + layer_loc))
        } else if let Some(ret) = self
            .space
            .elements()
            .rev()
            .filter(|win| win.is_on_active_tag(self.space.outputs()))
            .find_map(|win| {
                let loc = self
                    .space
                    .element_location(win)
                    .expect("called elem loc on unmapped win")
                    - win.geometry().loc;

                win.is_in_input_region(&(point - loc.to_f64()))
                    .then(|| (win.clone().into(), loc))
            })
        {
            Some(ret)
        } else if let (Some(layer), _) | (None, Some(layer)) = (
            layers.layer_under(wlr_layer::Layer::Overlay, point),
            layers.layer_under(wlr_layer::Layer::Top, point),
        ) {
            let layer_loc = layers.layer_geometry(layer).expect("no layer geo").loc;
            Some((FocusTarget::from(layer.clone()), output_geo.loc + layer_loc))
        } else {
            None
        }
    }

    fn keyboard<I: InputBackend>(&mut self, event: I::KeyboardKeyEvent) {
        let serial = SERIAL_COUNTER.next_serial();
        let time = event.time_msec();
        let press_state = event.state();

        let reload_keybind = self.input_state.reload_keybind;
        let kill_keybind = self.input_state.kill_keybind;

        let keyboard = self.seat.get_keyboard().expect("Seat has no keyboard");

        let modifiers = keyboard.modifier_state();

        let mut leds = Led::empty();
        if modifiers.num_lock {
            leds |= Led::NUMLOCK;
        }
        if modifiers.caps_lock {
            leds |= Led::CAPSLOCK;
        }

        // FIXME: Leds only update once another key is pressed.
        for device in self.input_state.libinput_devices.iter_mut() {
            device.led_update(leds);
        }

        let action = keyboard.input(
            self,
            event.key_code(),
            press_state,
            serial,
            time,
            |state, modifiers, keysym| {
                if press_state == KeyState::Pressed {
                    let mut modifier_mask = Vec::<Modifier>::new();
                    if modifiers.alt {
                        modifier_mask.push(Modifier::Alt);
                    }
                    if modifiers.shift {
                        modifier_mask.push(Modifier::Shift);
                    }
                    if modifiers.ctrl {
                        modifier_mask.push(Modifier::Ctrl);
                    }
                    if modifiers.logo {
                        modifier_mask.push(Modifier::Super);
                    }
                    let modifier_mask = ModifierMask::from(modifier_mask);

                    let raw_sym = keysym.raw_syms().iter().next();
                    let mod_sym = keysym.modified_sym();

                    let cb_id_mod = state.input_state.keybinds.get(&(modifier_mask, mod_sym));

                    let cb_id_raw = raw_sym.and_then(|raw_sym| {
                        state.input_state.keybinds.get(&(modifier_mask, *raw_sym))
                    });

                    match (cb_id_mod, cb_id_raw) {
                        (Some(cb_id), _) | (None, Some(cb_id)) => {
                            return FilterResult::Intercept(KeyAction::CallCallback(*cb_id));
                        }
                        (None, None) => (),
                    }

                    if kill_keybind == Some((modifier_mask, mod_sym)) {
                        return FilterResult::Intercept(KeyAction::Quit);
                    } else if reload_keybind == Some((modifier_mask, mod_sym)) {
                        return FilterResult::Intercept(KeyAction::ReloadConfig);
                    } else if let mut vt @ keysyms::KEY_XF86Switch_VT_1
                        ..=keysyms::KEY_XF86Switch_VT_12 = keysym.modified_sym().raw()
                    {
                        vt = vt - keysyms::KEY_XF86Switch_VT_1 + 1;
                        tracing::info!("Switching to vt {vt}");
                        return FilterResult::Intercept(KeyAction::SwitchVt(vt as i32));
                    }
                }

                FilterResult::Forward
            },
        );

        match action {
            Some(KeyAction::CallCallback(callback_id)) => {
                if let Some(stream) = self.api_state.stream.as_ref() {
                    if let Err(err) = crate::api::send_to_client(
                        &mut stream.lock().expect("Could not lock stream mutex"),
                        &OutgoingMsg::CallCallback {
                            callback_id,
                            args: None,
                        },
                    ) {
                        tracing::error!("error sending msg to client: {err}");
                    }
                }
            }
            Some(KeyAction::SwitchVt(vt)) => {
                self.switch_vt(vt);
            }
            Some(KeyAction::Quit) => {
                tracing::info!("Quitting Pinnacle");
                self.loop_signal.stop();
            }
            Some(KeyAction::ReloadConfig) => {
                self.start_config(crate::config::get_config_dir())
                    .expect("failed to restart config");
            }
            None => (),
        }
    }

    fn pointer_button<I: InputBackend>(&mut self, event: I::PointerButtonEvent) {
        let pointer = self.seat.get_pointer().expect("Seat has no pointer"); // FIXME: handle err
        let keyboard = self.seat.get_keyboard().expect("Seat has no keyboard"); // FIXME: handle err

        let serial = SERIAL_COUNTER.next_serial();

        let button = event.button_code();

        let button_state = event.state();

        let pointer_loc = pointer.current_location();

        let mouse_edge = match button_state {
            ButtonState::Released => MouseEdge::Release,
            ButtonState::Pressed => MouseEdge::Press,
        };
        let modifier_mask = ModifierMask::from(keyboard.modifier_state());

        // If any mousebinds are detected, call the config's callback and return.
        if let Some(&callback_id) =
            self.input_state
                .mousebinds
                .get(&(modifier_mask, button, mouse_edge))
        {
            if let Some(stream) = self.api_state.stream.as_ref() {
                crate::api::send_to_client(
                    &mut stream.lock().expect("failed to lock api stream"),
                    &OutgoingMsg::CallCallback {
                        callback_id,
                        args: None,
                    },
                )
                .expect("failed to call callback");
            }
            return;
        }

        // If the button was clicked, focus on the window below if exists, else
        // unfocus on windows.
        if button_state == ButtonState::Pressed {
            if let Some((focus, _)) = self.surface_under(pointer_loc) {
                // Move window to top of stack.
                if let FocusTarget::Window(window) = &focus {
                    self.space.raise_element(window, true);
                    if let WindowElement::X11(surface) = &window {
                        self.xwm
                            .as_mut()
                            .expect("no xwm")
                            .raise_window(surface)
                            .expect("failed to raise x11 win");
                        surface
                            .set_activated(true)
                            .expect("failed to set x11 win to activated");
                    }
                }

                tracing::debug!("wl_surface focus is some? {}", focus.wl_surface().is_some());

                // NOTE: *Do not* set keyboard focus to an override redirect window. This leads
                // |     to wonky things like right-click menus not correctly getting pointer
                // |     clicks or showing up at all.

                // TODO: use update_keyboard_focus from anvil

                if !matches!(
                    &focus,
                    FocusTarget::Window(WindowElement::X11OverrideRedirect(_))
                ) {
                    keyboard.set_focus(self, Some(focus.clone()), serial);
                }

                self.space.elements().for_each(|window| {
                    if let WindowElement::Wayland(window) = window {
                        window.toplevel().send_configure();
                    }
                });

                if let FocusTarget::Window(window) = &focus {
                    tracing::debug!("setting keyboard focus to {:?}", window.class());
                }
            } else {
                self.space.elements().for_each(|window| match window {
                    WindowElement::Wayland(window) => {
                        window.set_activated(false);
                        window.toplevel().send_configure();
                    }
                    WindowElement::X11(surface) => {
                        surface
                            .set_activated(false)
                            .expect("failed to deactivate x11 win");
                        // INFO: do i need to configure this?
                    }
                    WindowElement::X11OverrideRedirect(_) => (),
                });
                keyboard.set_focus(self, None, serial);
            }
        };

        pointer.button(
            self,
            &ButtonEvent {
                button,
                state: button_state,
                serial,
                time: event.time_msec(),
            },
        );
        pointer.frame(self);
    }

    fn pointer_axis<I: InputBackend>(&mut self, event: I::PointerAxisEvent) {
        let source = event.source();

        let horizontal_amount = event
            .amount(Axis::Horizontal)
            .unwrap_or_else(|| event.amount_discrete(Axis::Horizontal).unwrap_or(0.0) * 3.0);

        let vertical_amount = event
            .amount(Axis::Vertical)
            .unwrap_or_else(|| event.amount_discrete(Axis::Vertical).unwrap_or(0.0) * 3.0);

        let horizontal_amount_discrete = event.amount_discrete(Axis::Horizontal);
        let vertical_amount_discrete = event.amount_discrete(Axis::Vertical);

        let mut frame = AxisFrame::new(event.time_msec()).source(source);

        if horizontal_amount != 0.0 {
            frame = frame.value(Axis::Horizontal, horizontal_amount);
            if let Some(discrete) = horizontal_amount_discrete {
                frame = frame.discrete(Axis::Horizontal, discrete as i32);
            }
        } else if source == AxisSource::Finger {
            frame = frame.stop(Axis::Horizontal);
        }

        if vertical_amount != 0.0 {
            frame = frame.value(Axis::Vertical, vertical_amount);
            if let Some(discrete) = vertical_amount_discrete {
                frame = frame.discrete(Axis::Vertical, discrete as i32);
            }
        } else if source == AxisSource::Finger {
            frame = frame.stop(Axis::Vertical);
        }

        let pointer = self.seat.get_pointer().expect("Seat has no pointer");

        pointer.axis(self, frame);
        pointer.frame(self);
    }

    /// Clamp pointer coordinates inside outputs.
    ///
    /// This returns the nearest point inside an output.
    fn clamp_coords(&self, pos: Point<f64, Logical>) -> Point<f64, Logical> {
        if self.space.outputs().next().is_none() {
            return pos;
        }

        let (pos_x, pos_y) = pos.into();

        let nearest_points = self.space.outputs().map(|op| {
            let size = self
                .space
                .output_geometry(op)
                .expect("called output_geometry on unmapped output")
                .size;
            let loc = op.current_location();
            let pos_x = pos_x.clamp(loc.x as f64, (loc.x + size.w) as f64);
            let pos_y = pos_y.clamp(loc.y as f64, (loc.y + size.h) as f64);
            (pos_x, pos_y)
        });

        let nearest_point = nearest_points.min_by(|(x1, y1), (x2, y2)| {
            f64::total_cmp(
                &((pos_x - x1).powi(2) + (pos_y - y1).powi(2)).sqrt(),
                &((pos_x - x2).powi(2) + (pos_y - y2).powi(2)).sqrt(),
            )
        });

        nearest_point.map(|point| point.into()).unwrap_or(pos)
    }

    fn pointer_motion_absolute<I: InputBackend>(&mut self, event: I::PointerMotionAbsoluteEvent) {
        let Some(output) = self.space.outputs().next() else {
            return;
        };

        let output_geo = self
            .space
            .output_geometry(output)
            .expect("Output geometry doesn't exist");
        let pointer_loc = event.position_transformed(output_geo.size) + output_geo.loc.to_f64();
        let serial = SERIAL_COUNTER.next_serial();
        let pointer = self.seat.get_pointer().expect("Seat has no pointer"); // FIXME: handle err

        self.pointer_location = pointer_loc;

        match self.focus_state.focused_output {
            Some(_) => {
                if let Some(output) = self
                    .space
                    .output_under(self.pointer_location)
                    .next()
                    .cloned()
                {
                    self.focus_state.focused_output = Some(output);
                }
            }
            None => {
                self.focus_state.focused_output = self.space.outputs().next().cloned();
            }
        }

        pointer.motion(
            self,
            self.surface_under(pointer_loc),
            &MotionEvent {
                location: pointer_loc,
                serial,
                time: event.time_msec(),
            },
        );

        pointer.frame(self);
    }

    fn pointer_motion<I: InputBackend>(&mut self, event: I::PointerMotionEvent) {
        let serial = SERIAL_COUNTER.next_serial();
        self.pointer_location += event.delta();

        // clamp to screen limits
        // this event is never generated by winit
        self.pointer_location = self.clamp_coords(self.pointer_location);
        match self.focus_state.focused_output {
            Some(_) => {
                if let Some(output) = self
                    .space
                    .output_under(self.pointer_location)
                    .next()
                    .cloned()
                {
                    self.focus_state.focused_output = Some(output);
                }
            }
            None => {
                self.focus_state.focused_output = self.space.outputs().next().cloned();
            }
        }

        let surface_under = self.surface_under(self.pointer_location);

        if let Some(pointer) = self.seat.get_pointer() {
            pointer.motion(
                self,
                surface_under.clone(),
                &MotionEvent {
                    location: self.pointer_location,
                    serial,
                    time: event.time_msec(),
                },
            );

            pointer.relative_motion(
                self,
                surface_under,
                &RelativeMotionEvent {
                    delta: event.delta(),
                    delta_unaccel: event.delta_unaccel(),
                    utime: event.time(),
                },
            );

            pointer.frame(self);

            self.schedule_render(
                &self
                    .focus_state
                    .focused_output
                    .clone()
                    .expect("no focused output"),
            );
        }
    }
}
