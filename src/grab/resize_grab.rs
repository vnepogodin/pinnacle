// SPDX-License-Identifier: GPL-3.0-or-later

use smithay::{
    desktop::space::SpaceElement,
    input::{
        pointer::{AxisFrame, ButtonEvent, Focus, GrabStartData, PointerGrab, PointerInnerHandle},
        Seat, SeatHandler,
    },
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel::{self},
        wayland_server::protocol::wl_surface::WlSurface,
    },
    utils::{IsAlive, Logical, Point, Rectangle, Size},
    wayland::{compositor, shell::xdg::SurfaceCachedState},
    xwayland,
};

use crate::{
    backend::Backend,
    state::{State, WithState},
    window::{window_state::Status, WindowElement},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResizeEdge(pub xdg_toplevel::ResizeEdge);

impl From<xwayland::xwm::ResizeEdge> for ResizeEdge {
    fn from(value: xwayland::xwm::ResizeEdge) -> Self {
        match value {
            xwayland::xwm::ResizeEdge::Bottom => Self(xdg_toplevel::ResizeEdge::Bottom),
            xwayland::xwm::ResizeEdge::BottomLeft => Self(xdg_toplevel::ResizeEdge::BottomLeft),
            xwayland::xwm::ResizeEdge::BottomRight => Self(xdg_toplevel::ResizeEdge::BottomRight),
            xwayland::xwm::ResizeEdge::Left => Self(xdg_toplevel::ResizeEdge::Left),
            xwayland::xwm::ResizeEdge::Right => Self(xdg_toplevel::ResizeEdge::Right),
            xwayland::xwm::ResizeEdge::Top => Self(xdg_toplevel::ResizeEdge::Top),
            xwayland::xwm::ResizeEdge::TopLeft => Self(xdg_toplevel::ResizeEdge::TopLeft),
            xwayland::xwm::ResizeEdge::TopRight => Self(xdg_toplevel::ResizeEdge::TopRight),
        }
    }
}

impl From<xdg_toplevel::ResizeEdge> for ResizeEdge {
    fn from(value: xdg_toplevel::ResizeEdge) -> Self {
        Self(value)
    }
}

pub struct ResizeSurfaceGrab<S: SeatHandler> {
    start_data: GrabStartData<S>,
    window: WindowElement,

    edges: ResizeEdge,

    initial_window_rect: Rectangle<i32, Logical>,
    last_window_size: Size<i32, Logical>,

    button_used: u32,
}

impl<S: SeatHandler> ResizeSurfaceGrab<S> {
    pub fn start(
        start_data: GrabStartData<S>,
        window: WindowElement,
        edges: ResizeEdge,
        initial_window_rect: Rectangle<i32, Logical>,
        button_used: u32,
    ) -> Option<Self> {
        window.wl_surface()?.with_state(|state| {
            state.resize_state = ResizeSurfaceState::Resizing {
                edges,
                initial_window_rect,
            };
        });

        Some(Self {
            start_data,
            window,
            edges,
            initial_window_rect,
            last_window_size: initial_window_rect.size,
            button_used,
        })
    }
}

impl<B: Backend> PointerGrab<State<B>> for ResizeSurfaceGrab<State<B>> {
    fn motion(
        &mut self,
        data: &mut State<B>,
        handle: &mut PointerInnerHandle<'_, State<B>>,
        _focus: Option<(<State<B> as SeatHandler>::PointerFocus, Point<i32, Logical>)>,
        event: &smithay::input::pointer::MotionEvent,
    ) {
        handle.motion(data, None, event);

        if !self.window.alive() {
            handle.unset_grab(data, event.serial, event.time);
            return;
        }

        let delta = (event.location - self.start_data.location).to_i32_round::<i32>();

        let mut new_window_width = self.initial_window_rect.size.w;
        let mut new_window_height = self.initial_window_rect.size.h;

        if let xdg_toplevel::ResizeEdge::Left
        | xdg_toplevel::ResizeEdge::TopLeft
        | xdg_toplevel::ResizeEdge::BottomLeft = self.edges.0
        {
            new_window_width = self.initial_window_rect.size.w - delta.x;
        }
        if let xdg_toplevel::ResizeEdge::Right
        | xdg_toplevel::ResizeEdge::TopRight
        | xdg_toplevel::ResizeEdge::BottomRight = self.edges.0
        {
            new_window_width = self.initial_window_rect.size.w + delta.x;
        }
        if let xdg_toplevel::ResizeEdge::Top
        | xdg_toplevel::ResizeEdge::TopRight
        | xdg_toplevel::ResizeEdge::TopLeft = self.edges.0
        {
            new_window_height = self.initial_window_rect.size.h - delta.y;
        }
        if let xdg_toplevel::ResizeEdge::Bottom
        | xdg_toplevel::ResizeEdge::BottomRight
        | xdg_toplevel::ResizeEdge::BottomLeft = self.edges.0
        {
            new_window_height = self.initial_window_rect.size.h + delta.y;
        }

        let (min_size, max_size) = match self.window.wl_surface() {
            Some(wl_surface) => compositor::with_states(&wl_surface, |states| {
                let data = states.cached_state.current::<SurfaceCachedState>();
                (data.min_size, data.max_size)
            }),
            None => ((0, 0).into(), (0, 0).into()),
        };

        // HACK: Here I set the min height to be self.window.geometry().loc.y.abs() because if it's
        // |     lower then the compositor crashes trying to create a size with height -1 if you make the
        // |     window height too small.
        // |     However I don't know if the loc.y from window.geometry will always be the negative
        // |     of the csd height.
        let min_width = i32::max(1, min_size.w);
        let min_height = i32::max(
            i32::max(0, self.window.geometry().loc.y.abs()) + 1,
            min_size.h,
        );

        let max_width = if max_size.w != 0 { max_size.w } else { i32::MAX };
        let max_height = if max_size.h != 0 { max_size.h } else { i32::MAX };

        self.last_window_size = Size::from((
            new_window_width.clamp(min_width, max_width),
            new_window_height.clamp(min_height, max_height),
        ));

        match &self.window {
            WindowElement::Wayland(window) => {
                let toplevel_surface = window.toplevel();

                toplevel_surface.with_pending_state(|state| {
                    state.states.set(xdg_toplevel::State::Resizing);
                    state.size = Some(self.last_window_size);
                });

                toplevel_surface.send_pending_configure();
            }
            WindowElement::X11(surface) => {
                let loc = data
                    .space
                    .element_location(&self.window)
                    .expect("failed to get x11 win loc");
                surface
                    .configure(Rectangle::from_loc_and_size(loc, self.last_window_size))
                    .expect("failed to configure x11 win");
            }
        }
    }

    fn relative_motion(
        &mut self,
        data: &mut State<B>,
        handle: &mut PointerInnerHandle<'_, State<B>>,
        focus: Option<(<State<B> as SeatHandler>::PointerFocus, Point<i32, Logical>)>,
        event: &smithay::input::pointer::RelativeMotionEvent,
    ) {
        handle.relative_motion(data, focus, event);
    }

    fn button(
        &mut self,
        data: &mut State<B>,
        handle: &mut PointerInnerHandle<'_, State<B>>,
        event: &ButtonEvent,
    ) {
        handle.button(data, event);

        if !handle.current_pressed().contains(&self.button_used) {
            handle.unset_grab(data, event.serial, event.time);

            if !self.window.alive() {
                return;
            }

            match &self.window {
                WindowElement::Wayland(window) => {
                    let toplevel_surface = window.toplevel();
                    toplevel_surface.with_pending_state(|state| {
                        state.states.unset(xdg_toplevel::State::Resizing);
                        state.size = Some(self.last_window_size);
                    });

                    toplevel_surface.send_pending_configure();

                    toplevel_surface.wl_surface().with_state(|state| {
                        // TODO: validate resize state
                        state.resize_state = ResizeSurfaceState::WaitingForLastCommit {
                            edges: self.edges,
                            initial_window_rect: self.initial_window_rect,
                        };
                    });
                }
                WindowElement::X11(surface) => {
                    let Some(surface) = surface.wl_surface() else { return };
                    surface.with_state(|state| {
                        state.resize_state = ResizeSurfaceState::WaitingForLastCommit {
                            edges: self.edges,
                            initial_window_rect: self.initial_window_rect,
                        };
                    });
                }
            }
        }
    }

    fn axis(
        &mut self,
        data: &mut State<B>,
        handle: &mut PointerInnerHandle<'_, State<B>>,
        details: AxisFrame,
    ) {
        handle.axis(data, details);
    }

    fn start_data(&self) -> &GrabStartData<State<B>> {
        &self.start_data
    }
}

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq)]
pub enum ResizeSurfaceState {
    #[default]
    Idle,
    Resizing {
        edges: ResizeEdge,
        initial_window_rect: Rectangle<i32, Logical>,
    },
    WaitingForLastCommit {
        edges: ResizeEdge,
        initial_window_rect: Rectangle<i32, Logical>,
    },
}

impl ResizeSurfaceState {
    fn commit(&mut self) -> Option<(ResizeEdge, Rectangle<i32, Logical>)> {
        match *self {
            Self::Idle => None,
            Self::Resizing {
                edges,
                initial_window_rect,
            } => Some((edges, initial_window_rect)),
            Self::WaitingForLastCommit {
                edges,
                initial_window_rect,
            } => {
                *self = Self::Idle;
                Some((edges, initial_window_rect))
            }
        }
    }
}

pub fn handle_commit<B: Backend>(state: &mut State<B>, surface: &WlSurface) -> Option<()> {
    let window = state.window_for_surface(surface)?;
    let mut window_loc = state.space.element_location(&window)?;
    let geometry = window.geometry();

    let new_loc: Point<Option<i32>, Logical> = surface.with_state(|state| {
        state
            .resize_state
            .commit()
            .map(|(edges, initial_window_rect)| {
                let mut new_x: Option<i32> = None;
                let mut new_y: Option<i32> = None;
                if let xdg_toplevel::ResizeEdge::Left
                | xdg_toplevel::ResizeEdge::TopLeft
                | xdg_toplevel::ResizeEdge::BottomLeft = edges.0
                {
                    new_x = Some(
                        initial_window_rect.loc.x + (initial_window_rect.size.w - geometry.size.w),
                    );
                }
                if let xdg_toplevel::ResizeEdge::Top
                | xdg_toplevel::ResizeEdge::TopLeft
                | xdg_toplevel::ResizeEdge::TopRight = edges.0
                {
                    new_y = Some(
                        initial_window_rect.loc.y + (initial_window_rect.size.h - geometry.size.h),
                    );
                }

                (new_x, new_y)
            })
            .unwrap_or_default()
            .into()
    });

    if let Some(new_x) = new_loc.x {
        window_loc.x = new_x;
    }
    if let Some(new_y) = new_loc.y {
        window_loc.y = new_y;
    }

    if new_loc.x.is_some() || new_loc.y.is_some() {
        state.space.map_element(window.clone(), window_loc, false);
        let size = state
            .space
            .element_geometry(&window)
            .expect("called element_geometry on unmapped window")
            .size;

        window.with_state(|state| {
            if state.status.is_floating() {
                state.status = Status::Floating(Rectangle::from_loc_and_size(window_loc, size));
            }
        });

        if let WindowElement::X11(surface) = window {
            let geo = surface.geometry();
            let new_geo = Rectangle::from_loc_and_size(window_loc, geo.size);
            surface
                .configure(new_geo)
                .expect("failed to configure x11 win");
        }
    }

    Some(())
}

pub fn resize_request_client<B: Backend>(
    state: &mut State<B>,
    surface: &WlSurface,
    seat: &Seat<State<B>>,
    serial: smithay::utils::Serial,
    edges: self::ResizeEdge,
    button_used: u32,
) {
    let pointer = seat.get_pointer().expect("seat had no pointer");

    if let Some(start_data) = crate::pointer::pointer_grab_start_data(&pointer, surface, serial) {
        let Some(window) = state.window_for_surface(surface) else {
            tracing::error!("Surface had no window, cancelling resize request");
            return;
        };

        if window.with_state(|state| state.status.is_tiled()) {
            return;
        }

        let initial_window_loc = state
            .space
            .element_location(&window)
            .expect("resize request called on unmapped window");
        let initial_window_size = window.geometry().size;

        if let Some(WindowElement::Wayland(window)) = state.window_for_surface(surface) {
            window.toplevel().with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Resizing);
            });

            window.toplevel().send_pending_configure();
        }

        let grab = ResizeSurfaceGrab::start(
            start_data,
            window,
            edges,
            Rectangle::from_loc_and_size(initial_window_loc, initial_window_size),
            button_used,
        );

        if let Some(grab) = grab {
            pointer.set_grab(state, grab, serial, Focus::Clear);
        }
    }
}

pub fn resize_request_server<B: Backend>(
    state: &mut State<B>,
    surface: &WlSurface,
    seat: &Seat<State<B>>,
    serial: smithay::utils::Serial,
    edges: self::ResizeEdge,
    button_used: u32,
) {
    let pointer = seat.get_pointer().expect("seat had no pointer");

    let Some(window) = state.window_for_surface(surface) else {
        tracing::error!("Surface had no window, cancelling resize request");
        return;
    };

    if window.with_state(|state| state.status.is_tiled()) {
        return;
    }

    let initial_window_loc = state
        .space
        .element_location(&window)
        .expect("resize request called on unmapped window");
    let initial_window_size = window.geometry().size;

    if let Some(WindowElement::Wayland(window)) = state.window_for_surface(surface) {
        window.toplevel().with_pending_state(|state| {
            state.states.set(xdg_toplevel::State::Resizing);
        });

        window.toplevel().send_pending_configure();
    }

    let start_data = smithay::input::pointer::GrabStartData {
        focus: pointer
            .current_focus()
            .map(|focus| (focus, initial_window_loc)),
        button: button_used,
        location: pointer.current_location(),
    };

    let grab = ResizeSurfaceGrab::start(
        start_data,
        window,
        edges,
        Rectangle::from_loc_and_size(initial_window_loc, initial_window_size),
        button_used,
    );

    if let Some(grab) = grab {
        pointer.set_grab(state, grab, serial, Focus::Clear);
    }
}
