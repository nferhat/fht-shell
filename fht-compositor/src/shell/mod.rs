pub mod cursor;
pub mod decorations;
pub mod focus_target;
pub mod grabs;
pub mod window;
pub mod workspaces;

use std::time::Duration;

use smithay::desktop::space::SpaceElement;
use smithay::desktop::{
    find_popup_root_surface, get_popup_toplevel_coords, layer_map_for_output, PopupKind,
    WindowSurfaceType,
};
use smithay::input::pointer::Focus;
use smithay::output::Output;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::Resource;
use smithay::utils::{Point, Rectangle, Serial, Size};
use smithay::wayland::compositor::with_states;
use smithay::wayland::seat::WaylandFocus;
use smithay::wayland::shell::wlr_layer::Layer;
use smithay::wayland::shell::xdg::{PopupSurface, SurfaceCachedState};
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode as DecorationMode;

pub use self::focus_target::{KeyboardFocusTarget, PointerFocusTarget};
use self::grabs::MoveSurfaceGrab;
pub use self::window::FhtWindow;
use self::workspaces::{FullscreenSurface, Workspace, WorkspaceSwitchAnimation};
use crate::config::{WindowMapSettings, CONFIG};
use crate::state::{Fht, State};
use crate::utils::geometry::{
    Global, Local, PointExt, PointGlobalExt, PointLocalExt, RectCenterExt, RectExt, RectGlobalExt,
    RectLocalExt, SizeExt,
};
use crate::utils::output::OutputExt;

impl Fht {
    /// Get the [`FocusTarget`] under the cursor.
    ///
    /// It checks the surface under the cursor using the following order:
    /// - [`Overlay`] layer shells.
    /// - [`Fullscreen`] windows on the active workspace.
    /// - [`Top`] layer shells.
    /// - Normal/Maximized windows on the active workspace.
    /// - [`Bottom`] layer shells.
    /// - [`Background`] layer shells.
    pub fn focus_target_under(
        &self,
        point: Point<f64, Global>,
    ) -> Option<(PointerFocusTarget, Point<i32, Global>)> {
        let output = self.focus_state.output.as_ref()?;
        let wset = self.wset_for(output);
        let layer_map = layer_map_for_output(output);

        let mut under = None;

        if let Some(layer) = layer_map.layer_under(Layer::Overlay, point.as_logical()) {
            let layer_loc = layer_map.layer_geometry(layer).unwrap().loc.as_local();
            under = Some((layer.clone().into(), layer_loc.to_global(output)))
        } else if let Some((fullscreen, loc)) = wset.current_fullscreen() {
            under = Some((fullscreen.clone().into(), loc))
        } else if let Some(layer) = layer_map.layer_under(Layer::Top, point.as_logical()) {
            let layer_loc = layer_map.layer_geometry(layer).unwrap().loc.as_local();
            under = Some((layer.clone().into(), layer_loc.to_global(output)))
        } else if let Some((window, loc)) = wset.window_under(point) {
            under = Some((window.clone().into(), loc))
        } else if let Some(layer) = layer_map
            .layer_under(Layer::Bottom, point.as_logical())
            .or_else(|| layer_map.layer_under(Layer::Background, point.as_logical()))
        {
            let layer_loc = layer_map.layer_geometry(layer).unwrap().loc.as_local();
            under = Some((layer.clone().into(), layer_loc.to_global(output)))
        }

        under
    }

    /// Find the window associated with this [`WlSurface`]
    pub fn find_window(&self, surface: &WlSurface) -> Option<&FhtWindow> {
        self.workspaces()
            .find_map(|(_, wset)| wset.find_window(surface))
    }

    /// Find the window associated with this [`WlSurface`], and the output the window is mapped
    /// onto
    pub fn find_window_and_output(&self, surface: &WlSurface) -> Option<(&FhtWindow, &Output)> {
        self.workspaces()
            .find_map(|(_, wset)| wset.find_window(surface).map(|w| (w, &wset.output)))
    }

    /// Get a reference to the workspace holding this window
    pub fn ws_for(&self, window: &FhtWindow) -> Option<&Workspace> {
        self.workspaces().find_map(|(_, wset)| wset.ws_for(window))
    }

    /// Get a mutable reference to the workspace holding this window
    pub fn ws_mut_for(&mut self, window: &FhtWindow) -> Option<&mut Workspace> {
        self.workspaces_mut()
            .find_map(|(_, wset)| wset.ws_mut_for(window))
    }

    /// Find the first output where this [`WlSurface`] is visible.
    ///
    /// This checks everything from layer shells to windows to override redirect windows etc.
    pub fn visible_output_for_surface(&self, surface: &WlSurface) -> Option<&Output> {
        self.outputs()
            .find(|o| {
                // Is the surface a layer shell?
                let layer_map = layer_map_for_output(o);
                layer_map
                    .layer_for_surface(surface, WindowSurfaceType::ALL)
                    .is_some()
            })
            .or_else(|| {
                // Pending layer_surface?
                self.pending_layers.iter().find_map(|(_, (l, output))| {
                    let mut found = false;
                    l.with_surfaces(|s, _| {
                        if s == surface {
                            found = true;
                        }
                    });
                    found.then_some(output)
                })
            })
            .or_else(|| {
                // Mapped window?
                self.workspaces().find_map(|(o, wset)| {
                    let active = wset.active();
                    if active
                        .windows
                        .iter()
                        .any(|w| w.has_surface(surface, WindowSurfaceType::ALL))
                    {
                        return Some(o);
                    }

                    if active
                        .fullscreen
                        .as_ref()
                        .is_some_and(|f| f.inner.has_surface(surface, WindowSurfaceType::ALL))
                    {
                        return Some(o);
                    }

                    None
                })
            })
    }

    /// Find every output where this window (and it's subsurfaces) is displayed.
    pub fn visible_outputs_for_window(&self, window: &FhtWindow) -> impl Iterator<Item = &Output> {
        let window_geo = window.global_geometry();
        self.outputs()
            .filter(move |o| o.geometry().intersection(window_geo).is_some())
    }

    /// Find every window that is curently displayed on this output
    #[profiling::function]
    pub fn visible_windows_for_output(
        &self,
        output: &Output,
    ) -> Box<dyn Iterator<Item = &FhtWindow> + '_> {
        let wset = self.wset_for(output);

        if let Some(WorkspaceSwitchAnimation { target_idx, .. }) = wset.switch_animation.as_ref() {
            let active = wset.active();
            let target = &wset.workspaces[*target_idx];
            if let Some(fullscreen) = active
                .fullscreen
                .as_ref()
                .map(|f| &f.inner)
                .or_else(|| target.fullscreen.as_ref().map(|f| &f.inner))
            {
                return Box::new(std::iter::once(fullscreen))
                    as Box<dyn Iterator<Item = &FhtWindow>>;
            } else {
                return Box::new(active.windows.iter().chain(target.windows.iter()))
                    as Box<dyn Iterator<Item = &FhtWindow>>;
            }
        } else {
            let active = wset.active();
            if let Some(fullscreen) = active.fullscreen.as_ref().map(|f| &f.inner) {
                return Box::new(std::iter::once(fullscreen))
                    as Box<dyn Iterator<Item = &FhtWindow>>;
            } else {
                return Box::new(active.windows.iter()) as Box<dyn Iterator<Item = &FhtWindow>>;
            }
        }
    }

    /// Prepapre a pending window to be mapped.
    pub fn prepare_map_window(&mut self, window: &FhtWindow) {
        let mut output = self.focus_state.output.clone().unwrap();
        let wl_surface = window.wl_surface().unwrap();

        // Get the matching mapping setting, if the user specified one.
        let workspace_idx = self.wset_for(&output).get_active_idx();
        let map_settings = CONFIG
            .rules
            .iter()
            .find(|(rules, _)| rules.iter().any(|r| r.matches(&window, workspace_idx)))
            .map(|(_, settings)| settings.clone())
            .unwrap_or_default();

        // Apply rules
        //
        // First start with the output since every operation (mapping,  fullscreening, etc...) will
        // be done relative to the output.
        if let Some(target_output) = map_settings
            .output
            .as_ref()
            .and_then(|name| self.outputs().find(|o| o.name().as_str() == name))
            .cloned()
        {
            output = target_output;
        }

        let workspace_idx = match map_settings.workspace {
            None => self.wset_for(&output).get_active_idx(),
            Some(idx) => idx.clamp(0, 9),
        };

        let dh = self.display_handle.clone();
        let wset = self.wset_mut_for(&output);
        let workspace = &mut wset.workspaces[workspace_idx];

        // TODO: Logic here....
        if map_settings.fullscreen {
            // Use output geometry, and account for window borders.
            window.set_geometry(output.geometry(), false);
            window.set_tiled(!map_settings.floating);

            let mut wl_output = None;
            let client = dh.get_client(wl_surface.id()).unwrap();
            for wl_output_2 in output.client_outputs(&client) {
                wl_output = Some(wl_output_2);
            }

            window.set_fullscreen(true, wl_output);
        } else if map_settings.floating {
            // Sadly we can't pre-compute the size for floating windows.
            //
            // Floating windows can take a `centered` rule, that needs the window size to be set,
            // which only happens when sending the initial configure message. We take care of this
            // in `Fht::map_window`
            window.set_tiled(false);
        } else {
            window.set_tiled(true);

            // The following code is adapted from refresh_window_geometries.
            //
            // It uses the workspace layout to pre-shape the window using tile_windows, since it
            // accepts an iterator
            let (maximized_windows, mut tiled_windows): (Vec<&FhtWindow>, Vec<&FhtWindow>) =
                workspace.windows.iter().partition(|w| w.is_maximized());
            tiled_windows.retain(|w| w.is_tiled());

            let inner_gaps = CONFIG.general.inner_gaps;
            let outer_gaps = CONFIG.general.outer_gaps;

            let output_geo = output.geometry();
            if let Some(window) = workspace.fullscreen.as_ref().map(|f| &f.inner) {
                window.set_geometry(output_geo, false);
                if let Some(toplevel) = window.0.toplevel() {
                    toplevel.send_pending_configure();
                }
            }

            let usable_geo = layer_map_for_output(&output)
                .non_exclusive_zone()
                .as_local()
                .to_global(&output);
            let mut maximized_geo = usable_geo;
            maximized_geo.size -= (2 * outer_gaps, 2 * outer_gaps).into();
            maximized_geo.loc += (outer_gaps, outer_gaps).into();
            for window in maximized_windows {
                window.set_geometry(maximized_geo, true);
                if let Some(toplevel) = window.0.toplevel() {
                    toplevel.send_pending_configure();
                }
            }

            // Can't check whether we are empty cause we are always not gonna be empty
            // (we are inserting a window, soo the iterator has always one item)
            let windows_len = tiled_windows.len();
            workspace.get_active_layout().tile_windows(
                tiled_windows.into_iter().chain(std::iter::once(window)),
                windows_len + 1,
                maximized_geo,
                inner_gaps,
                |_idx, w, new_geo| {
                    w.set_geometry(new_geo, true);
                    if w != window {
                        // Do not send a configure for the prepared window since we are sending
                        // changes with an initial configure call
                        if let Some(toplevel) = w.0.toplevel() {
                            toplevel.send_pending_configure();
                        }
                    }
                },
            );
        }

        // Client side-decorations
        let allow_csd = map_settings
            .allow_csd
            .unwrap_or(CONFIG.decoration.allow_csd);
        if let Some(toplevel) = window.0.toplevel() {
            toplevel.with_pending_state(|state| {
                if allow_csd {
                    state.decoration_mode = Some(DecorationMode::ClientSide)
                } else {
                    state.decoration_mode = Some(DecorationMode::ServerSide)
                }
            });
        }
        // NOTE: We can't do much on x11...

        let map_settings = WindowMapSettingsInternal {
            output,
            workspace_idx,
            user: map_settings,
        };

        window.user_data().insert_if_missing(|| map_settings);
        if let Some(toplevel) = window.0.toplevel() {
            toplevel.send_configure();
        }
    }

    /// Map a pending window, if it's found.
    pub fn map_window(&mut self, window: FhtWindow) {
        let settings = window
            .user_data()
            .get::<WindowMapSettingsInternal>()
            .expect("Tried to map_window without prepare_map_window!");

        let wl_surface = window.wl_surface().unwrap();
        let wset = self.wset_mut_for(&settings.output);
        let is_active = settings.workspace_idx == wset.get_active_idx();
        let workspace = &mut wset.workspaces[settings.workspace_idx];

        if settings.user.fullscreen {
            // Fullscreen windows are not in the window vector, they must be inserted otherwise.
            workspace.fullscreen = Some(FullscreenSurface {
                inner: window.clone(),
                last_known_idx: workspace.windows.len(),
            });

            if is_active {
                self.focus_state.focus_target = Some(window.into())
            }

            return;
        } else if settings.user.floating {
            let mut window_geo = Rectangle::default();
            if let Some(size) = settings.user.size.map(Into::into) {
                window_geo.size = size;
            } else {
                let min_size = with_states(&wl_surface, |states| {
                    states.cached_state.current::<SurfaceCachedState>().min_size
                });
                let space_element_size = window.geometry().size;
                let maybe_pending_size = window
                    .0
                    .toplevel()
                    .and_then(|t| t.with_pending_state(|state| state.size))
                    .filter(|&s| s != Size::default());

                if let Some(pending_size) = maybe_pending_size {
                    window_geo.size = pending_size.as_global();
                } else if space_element_size != Size::default() {
                    window_geo.size = space_element_size.as_global();
                } else if min_size.w > 20 && min_size.h > 20 {
                    window_geo.size = min_size.as_global();
                } else {
                    window_geo.size = (800, 600).into();
                }
            }

            if let Some(loc) = settings.user.location.map(Into::<Point<i32, Local>>::into) {
                window_geo.loc = loc.to_global(&settings.output);
            } else if settings.user.centered {
                let output_geo = settings.output.geometry();
                window_geo.loc = output_geo.loc + output_geo.size.downscale(2).to_point();
                window_geo.loc -= window_geo.size.downscale(2).to_point();
            }

            window.set_geometry(window_geo, true);
        }

        // Not fullscreen, insert as normal
        workspace.insert_window(window.clone());
        if settings.user.floating {
            // You probably want to see the window you just inserted, right?
            workspace.raise_window(&window);
        }

        // From using the compositor opening a window when a switch is being done feels more
        // natural when the window gets focus, even if focus_new_windows is none.
        let is_switching = wset.switch_animation.is_some();
        let should_focus = (CONFIG.general.focus_new_windows || is_switching) && is_active;

        workspace.insert_window(window.clone());
        if should_focus {
            if CONFIG.general.cursor_warps {
                let center = window.global_geometry().center();
                self.loop_handle
                    .insert_idle(move |state| state.move_pointer(center.to_f64()));
            }
            self.focus_state.focus_target = Some(window.into());
        }
    }

    /// Unconstraint a popup.
    ///
    /// Basically changes its geometry and location so that it doesn't overflow outside of the
    /// parent window's output.
    pub fn unconstrain_popup(&self, popup: &PopupSurface) {
        let Ok(root) = find_popup_root_surface(&PopupKind::Xdg(popup.clone())) else {
            return;
        };
        let Some(window) = self.find_window(&root) else {
            return;
        };

        let mut outputs_for_window = self.visible_outputs_for_window(window);
        if outputs_for_window.next().is_none() {
            return;
        }

        let mut outputs_geo = outputs_for_window
            .next()
            .unwrap_or_else(|| self.outputs().next().unwrap())
            .geometry();
        for output in outputs_for_window {
            outputs_geo = outputs_geo.merge(output.geometry());
        }

        // The target (aka the popup) geometry should be relative to the parent (aka the window's)
        // geometry, based on the xdg_shell protocol requirements.
        let mut target = outputs_geo;
        target.loc -= get_popup_toplevel_coords(&PopupKind::Xdg(popup.clone())).as_global();
        target.loc -= window.global_geometry().loc;

        popup.with_pending_state(|state| {
            state.geometry = state
                .positioner
                .get_unconstrained_geometry(target.as_logical());
        });
    }

    /// Advance all the active animations for this given output
    pub fn advance_animations(&mut self, output: &Output, current_time: Duration) {
        let wset = self.wset_mut_for(output);
        if let Some(WorkspaceSwitchAnimation { target_idx, .. }) =
            wset.switch_animation.take_if(|a| a.animation.is_finished())
        {
            wset.active_idx
                .store(target_idx, std::sync::atomic::Ordering::SeqCst);
        }
        if let Some(animation) = wset.switch_animation.as_mut() {
            animation.animation.set_current_time(current_time);
        }
    }

    /// Get an interator over all the windows registered in the compositor.
    pub fn all_windows(&self) -> impl Iterator<Item = &FhtWindow> + '_ {
        self.workspaces
            .values()
            .flat_map(|wset| wset.workspaces.iter().flat_map(|ws| &ws.windows))
    }
}

impl State {
    /// Process a move request for this given window.
    pub fn handle_move_request(&mut self, window: FhtWindow, serial: Serial) {
        // NOTE: About internal handling.
        // ---
        // Even though `XdgShellHandler::move_request` has a seat argument, we only advertise one
        // single seat to clients (why would we support multi-seat for a standalone compositor?)
        // So the only pointer we have is the advertised seat pointer.
        let pointer = self.fht.pointer.clone();
        if !pointer.has_grab(serial) {
            return;
        }
        let Some(start_data) = pointer.grab_start_data() else {
            return;
        };

        let Some(wl_surface) = window.wl_surface() else {
            return;
        };
        // Make sure we are moving the same window
        if start_data.focus.is_none()
            || !start_data
                .focus
                .as_ref()
                .unwrap()
                .0
                .same_client_as(&wl_surface.id())
        {
            return;
        }

        let window_geo = window.global_geometry();
        let mut initial_window_location = window_geo.loc;

        // Unmaximize/Unfullscreen if it already is.
        let is_maximized = window.is_maximized();
        let is_fullscreen = window.is_fullscreen();
        if is_maximized || is_fullscreen {
            window.set_maximized(false);
            window.set_fullscreen(false, None);
            if let Some(toplevel) = window.0.toplevel() {
                toplevel.send_configure();
            }

            // let pos = pointer.current_location().as_global();
            // let mut window_pos = pos - window_geo.to_f64().loc;
            // window_pos.x = window_pos.x.clamp(0.0, window_geo.size.w.to_f64());
            //
            // match window_pos.x / window_geo.size.w.to_f64() {
            //     x if x < 0.5
            // }
            let pos = pointer.current_location();
            initial_window_location = (pos.x as i32, pos.y as i32).into();
        }

        window.set_fullscreen(false, None);

        let grab = MoveSurfaceGrab {
            start_data,
            window,
            initial_window_location,
        };

        pointer.set_grab(self, grab, serial, Focus::Clear);
    }
}

/// Like [`WindowMapSettings`], but only holds concrete values.
///
/// As the suffix implies, this is for internal handling in [`Fht::map_window`]
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct WindowMapSettingsInternal {
    /// The output the window is getting mapped on
    output: Output,
    /// The workspace index the window is getting mapped on.
    workspace_idx: usize,
    /// User-specified settings.
    user: WindowMapSettings,
}
