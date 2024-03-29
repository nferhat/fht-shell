use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use anyhow::Context;
use indexmap::IndexMap;
use smithay::backend::renderer::element::utils::select_dmabuf_feedback;
use smithay::backend::renderer::element::{
    default_primary_scanout_output_compare, RenderElementStates,
};
use smithay::desktop::utils::{
    surface_presentation_feedback_flags_from_states, surface_primary_scanout_output,
    update_surface_primary_scanout_output, OutputPresentationFeedback,
};
use smithay::desktop::{layer_map_for_output, LayerSurface, PopupManager};
use smithay::input::keyboard::{KeyboardHandle, Keysym, XkbConfig};
use smithay::input::pointer::PointerHandle;
use smithay::input::{Seat, SeatState};
use smithay::output::Output;
use smithay::reexports::calloop::{self, LoopHandle, LoopSignal};
use smithay::reexports::input;
use smithay::reexports::wayland_server::backend::ClientData;
use smithay::reexports::wayland_server::protocol::wl_shm;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::utils::{Clock, IsAlive, Monotonic, Point, Rectangle, SERIAL_COUNTER};
use smithay::wayland::compositor::{CompositorClientState, CompositorState};
use smithay::wayland::dmabuf::{DmabufFeedback, DmabufState};
use smithay::wayland::fractional_scale::{with_fractional_scale, FractionalScaleManagerState};
use smithay::wayland::input_method::InputMethodManagerState;
use smithay::wayland::keyboard_shortcuts_inhibit::KeyboardShortcutsInhibitState;
use smithay::wayland::output::OutputManagerState;
use smithay::wayland::pointer_constraints::PointerConstraintsState;
use smithay::wayland::presentation::PresentationState;
use smithay::wayland::security_context::{SecurityContext, SecurityContextState};
use smithay::wayland::selection::data_device::DataDeviceState;
use smithay::wayland::selection::primary_selection::PrimarySelectionState;
use smithay::wayland::selection::wlr_data_control::DataControlState;
use smithay::wayland::shell::wlr_layer::WlrLayerShellState;
use smithay::wayland::shell::xdg::decoration::XdgDecorationState;
use smithay::wayland::shell::xdg::XdgShellState;
use smithay::wayland::shm::ShmState;
use smithay::wayland::tablet_manager::{TabletManagerState, TabletSeatTrait};
use smithay::wayland::text_input::TextInputManagerState;
use smithay::wayland::viewporter::ViewporterState;
use smithay::wayland::virtual_keyboard::VirtualKeyboardManagerState;
use smithay::wayland::xdg_activation::XdgActivationState;
use smithay_egui::EguiState;

use crate::backend::Backend;
use crate::config::CONFIG;
use crate::ipc::{IpcOutput, IpcOutputRequest};
use crate::shell::cursor::CursorThemeManager;
use crate::shell::workspaces::WorkspaceSet;
use crate::shell::{FhtWindow, KeyboardFocusTarget};
use crate::utils::dbus::DBUS_CONNECTION;
use crate::utils::geometry::{Global, RectCenterExt, SizeExt};
use crate::utils::output::OutputExt;
#[cfg(feature = "xdg-screencast-portal")]
use crate::utils::pipewire::PipeWire;

pub struct State {
    pub fht: Fht,
    pub backend: Backend,
}

impl State {
    /// Creates a new instance of the state.
    ///
    /// For backend initialization, use a module from [`crate::backend`] or use
    /// [`crate::backend::init_backend_auto`] to initiate an appropriate one.
    pub fn new(
        dh: &DisplayHandle,
        loop_handle: LoopHandle<'static, State>,
        loop_signal: LoopSignal,
        socket_name: String,
    ) -> Self {
        Self {
            fht: Fht::new(dh, loop_handle, loop_signal, socket_name),
            backend: Backend::None,
        }
    }

    /// Dispatch evenements from the wayland unix socket, have to be called on each evenement
    /// otherwise the events won't reach their target clients.
    #[profiling::function]
    pub fn dispatch(&mut self) -> anyhow::Result<()> {
        self.fht
            .workspaces_mut()
            .for_each(|(_, wset)| wset.refresh());
        self.fht.popups.cleanup();

        // Make sure the surface is not dead (otherwise wayland wont be happy)
        let _ = self.fht.focus_state.focus_target.take_if(|f| !f.alive());
        let old_focus = self.fht.keyboard.current_focus();
        {
            profiling::scope!("refresh_focus");
            if self.fht.focus_state.focus_target.is_some() {
                // If we need to focus a specific surface, then do it.
                if self.fht.focus_state.focus_target != old_focus {
                    self.fht.keyboard.clone().set_focus(
                        self,
                        self.fht.focus_state.focus_target.clone(),
                        SERIAL_COUNTER.next_serial(),
                    );
                }
            } else {
                // Otherwise, the focused target will be the focused window on the active workspace.
                if let Some(window) = self.fht.focus_state.output.as_ref().and_then(|o| {
                    let active = self.fht.wset_for(o).active();
                    active
                        .fullscreen
                        .as_ref()
                        .map(|f| f.inner.clone())
                        .or_else(|| active.focused().cloned())
                }) {
                    self.fht.focus_state.focus_target = Some(window.into());
                    self.fht.keyboard.clone().set_focus(
                        self,
                        self.fht.focus_state.focus_target.clone(),
                        SERIAL_COUNTER.next_serial(),
                    );
                }
            }
        }

        {
            profiling::scope!("DislpayHandle::flush_clients");
            self.fht
                .display_handle
                .flush_clients()
                .context("Failed to flush_clients!")?;
        }

        Ok(())
    }

    /// Create a new Wayland client state for a client stream bound to the WAYLAND_DISPLAY
    pub fn new_client_state(&self) -> ClientState {
        ClientState {
            compositor: CompositorClientState::default(),
            security_context: None,
        }
    }
}

pub struct Fht {
    pub socket_name: String,
    pub display_handle: DisplayHandle,
    pub loop_handle: LoopHandle<'static, State>,
    pub loop_signal: LoopSignal,
    pub stop: Arc<AtomicBool>,

    pub clock: Clock<Monotonic>,
    pub suppressed_keys: HashSet<Keysym>,
    pub seat: Seat<State>,
    pub tablet_cursor_location: Option<Point<i32, Global>>,
    pub devices: Vec<input::Device>,
    pub seat_state: SeatState<State>,
    pub keyboard: KeyboardHandle<State>,
    pub pointer: PointerHandle<State>,

    pub dnd_icon: Option<WlSurface>,
    pub cursor_theme_manager: CursorThemeManager,
    pub workspaces: IndexMap<Output, WorkspaceSet>,
    pub pending_layers: HashMap<WlSurface, (LayerSurface, Output)>,
    pub pending_windows: HashMap<WlSurface, FhtWindow>,
    pub focus_state: FocusState,
    pub popups: PopupManager,

    pub last_config_error: Option<anyhow::Error>,

    #[cfg(feature = "xdg-screencast-portal")]
    // We can't start PipeWire immediatly since pipewire may not be running yet, but when the
    // ScreenCast application starts it should be started by then.
    pub pipewire_initialised: std::sync::Once,
    #[cfg(feature = "xdg-screencast-portal")]
    pub pipewire: Option<PipeWire>,

    pub compositor_state: CompositorState,
    pub data_control_state: DataControlState,
    pub data_device_state: DataDeviceState,
    pub dmabuf_state: DmabufState,
    pub fractional_scale_manager_state: FractionalScaleManagerState,
    pub keyboard_shortcuts_inhibit_state: KeyboardShortcutsInhibitState,
    pub layer_shell_state: WlrLayerShellState,
    pub output_manager_state: OutputManagerState,
    pub presentation_state: PresentationState,
    pub primary_selection_state: PrimarySelectionState,
    pub shm_state: ShmState,
    pub viewporter_state: ViewporterState,
    pub xdg_activation_state: XdgActivationState,
    pub xdg_decoration_state: XdgDecorationState,
    pub xdg_shell_state: XdgShellState,
}

impl Fht {
    /// Create a new instance of the state, initializing all the wayland global objects
    pub fn new(
        dh: &DisplayHandle,
        loop_handle: LoopHandle<'static, State>,
        loop_signal: LoopSignal,
        socket_name: String,
    ) -> Self {
        let clock = Clock::<Monotonic>::new();
        info!("Initialized monotonic clock.");

        let compositor_state = CompositorState::new_v6::<State>(dh);
        let primary_selection_state = PrimarySelectionState::new::<State>(dh);
        let data_control_state =
            DataControlState::new::<State, _>(dh, Some(&primary_selection_state), |_| true);
        let data_device_state = DataDeviceState::new::<State>(dh);
        let dmabuf_state = DmabufState::new();
        let fractional_scale_manager_state = FractionalScaleManagerState::new::<State>(dh);
        let layer_shell_state = WlrLayerShellState::new::<State>(dh);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<State>(dh);
        let presentation_state = PresentationState::new::<State>(dh, clock.id() as u32);
        let shm_state =
            ShmState::new::<State>(dh, vec![wl_shm::Format::Xbgr8888, wl_shm::Format::Abgr8888]);
        let viewporter_state = ViewporterState::new::<State>(dh);
        let xdg_activation_state = XdgActivationState::new::<State>(dh);
        let xdg_decoration_state = XdgDecorationState::new::<State>(dh);
        let xdg_shell_state = XdgShellState::new::<State>(dh);
        TextInputManagerState::new::<State>(&dh);
        InputMethodManagerState::new::<State, _>(&dh, |_| true);
        VirtualKeyboardManagerState::new::<State, _>(&dh, |_| true);
        PointerConstraintsState::new::<State>(&dh);
        TabletManagerState::new::<State>(&dh);
        SecurityContextState::new::<State, _>(&dh, |client| {
            // From: https://wayland.app/protocols/security-context-v1
            // "Compositors should forbid nesting multiple security contexts"
            client
                .get_data::<ClientState>()
                .map_or(true, |data| data.security_context.is_none())
        });

        // Initialize a seat and immediatly attach a keyboard and pointer to it.
        // If clients try to connect and do not find any of them they will try to initialize them
        // themselves and chaos will endure.
        let mut seat_state = SeatState::new();
        let mut seat = seat_state.new_wl_seat(dh, "seat0");

        // Dont let the user crash the compositor with invalid config
        let keyboard_config = &CONFIG.input.keyboard;
        let res = seat.add_keyboard(
            keyboard_config.get_xkb_config(),
            keyboard_config.repeat_delay,
            keyboard_config.repeat_rate,
        );
        let keyboard = match res {
            Ok(k) => k,
            Err(err) => {
                error!(?err, "Failed to add keyboard! Falling back to defaults");
                seat.add_keyboard(
                    XkbConfig::default(),
                    keyboard_config.repeat_delay,
                    keyboard_config.repeat_rate,
                )
                .expect("The keyboard is not keyboarding")
            }
        };
        let pointer = seat.add_pointer();
        info!("Initialized wl_seat.");

        let cursor_theme_manager = CursorThemeManager::new();
        let cursor_image_status_clone = cursor_theme_manager.image_status.clone();
        seat.tablet_seat().on_cursor_surface(move |_, new_status| {
            *cursor_image_status_clone.lock().unwrap() = new_status;
        });

        let keyboard_shortcuts_inhibit_state = KeyboardShortcutsInhibitState::new::<State>(dh);

        Self {
            socket_name,
            display_handle: dh.clone(),
            loop_handle,
            loop_signal,
            stop: Arc::new(AtomicBool::new(false)),

            clock,
            suppressed_keys: HashSet::new(),
            seat,
            devices: vec![],
            tablet_cursor_location: None,
            seat_state,
            keyboard,
            pointer,
            focus_state: FocusState::default(),

            dnd_icon: None,
            cursor_theme_manager,
            workspaces: IndexMap::new(),
            pending_layers: HashMap::new(),
            pending_windows: HashMap::new(),
            popups: PopupManager::default(),

            last_config_error: None,

            #[cfg(feature = "xdg-screencast-portal")]
            pipewire_initialised: std::sync::Once::new(),
            #[cfg(feature = "xdg-screencast-portal")]
            pipewire: None,

            compositor_state,
            data_control_state,
            data_device_state,
            dmabuf_state,
            fractional_scale_manager_state,
            keyboard_shortcuts_inhibit_state,
            layer_shell_state,
            output_manager_state,
            presentation_state,
            primary_selection_state,
            shm_state,
            viewporter_state,
            xdg_activation_state,
            xdg_decoration_state,
            xdg_shell_state,
        }
    }
}

impl Fht {
    /// List all the registered outputs.
    pub fn outputs(&self) -> impl Iterator<Item = &Output> {
        self.workspaces.keys()
    }

    /// Handle an IPC output request.
    fn handle_ipc_output_request(&mut self, req: IpcOutputRequest, output: &Output) {
        match req {
            IpcOutputRequest::SetActiveWorkspaceIndex { index } => {
                self.wset_mut_for(output)
                    .set_active_idx(index as usize, true);
            }
        }
    }

    /// Register an output to the wayland state.
    ///
    /// # PANICS
    ///
    /// Trying to add the same output twice causes an assertion fail.
    pub fn add_output(&mut self, output: Output) {
        assert!(
            self.workspaces.get(&output).is_none(),
            "Tried to add an output twice!"
        );

        info!(name = output.name(), "Adding new output.");

        // Current default behaviour:
        //
        // When adding an output, put it to the right of every other output.
        // Right now this assumption can be false for alot of users, but this is just as a
        // fallback.
        //
        // TODO: Add output management config + wlr_output_management protocol.
        let x: i32 = self.outputs().map(|o| o.geometry().loc.x).sum();
        trace!(?x, y = 0, "Using fallback output location.");
        output.change_current_state(None, None, None, Some((x, 0).into()));

        let workspace_set = WorkspaceSet::new(output.clone(), self.loop_handle.clone());
        self.workspaces.insert(output.clone(), workspace_set);

        {
            let output = output.clone();
            let (ipc_output, ipc_path, from_ipc_channel) = IpcOutput::new(&output);

            self.loop_handle
                .insert_source(from_ipc_channel, move |event, _, state| {
                    let calloop::channel::Event::Msg(req) = event else {
                        return;
                    };
                    state.fht.handle_ipc_output_request(req, &output);
                })
                .expect("Failed to insert output IPC source!");

            assert!(DBUS_CONNECTION
                .object_server()
                .at(ipc_path, ipc_output)
                .unwrap());
        }

        // Focus output now.
        if CONFIG.general.cursor_warps {
            let center = output.geometry().center();
            self.loop_handle.insert_idle(move |state| {
                state.move_pointer(center.to_f64());
            });
        }
        self.focus_state.output = Some(output);
    }

    /// Unregister an output from the wayland state.
    ///
    /// # PANICS
    ///
    /// Trying remove a non-existent output causes an assertion fail.
    pub fn remove_output(&mut self, output: &Output) {
        info!(name = output.name(), "Removing output.");
        let removed_wset = self
            .workspaces
            .swap_remove(output)
            .expect("Tried to remove a non-existing output!");

        if self.workspaces.is_empty() {
            // There's nothing more todo, just adandon everything.
            self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
            return;
        }

        // Current behaviour:
        //
        // Move each window from each workspace in this removed output wset and bind it to the
        // first output available, very simple.
        //
        // In other words, if you had a window on ws1, 4, and 8 on this output, they would get
        // moved to their respective workspace on the first available wset.
        let wset = self.workspaces.first_mut().unwrap().1;

        for (mut old_workspace, new_workspace) in
            std::iter::zip(removed_wset.workspaces, wset.workspaces_mut())
        {
            // Little optimizaztion, to avoid recalculating window geometries each time
            //
            // Due to how we manage windows, a window can't be in two workspaces at a time, let
            // alone from different outputs
            new_workspace
                .windows
                .extend(old_workspace.windows.drain(..));
            new_workspace.refresh_window_geometries();
        }

        // Cleanly close [`LayerSurface`] instead of letting them know their demise after noticing
        // the output is gone.
        for layer in layer_map_for_output(output).layers() {
            layer.layer_surface().send_close()
        }

        // Unregister from IPC.
        {
            let path = format!(
                "/fht/desktop/Compositor/Output/{}",
                output.name().replace("-", "_")
            );
            match DBUS_CONNECTION
                .object_server()
                .remove::<crate::ipc::IpcOutput, _>(path)
            {
                Err(err) => warn!(?err, "Failed to de-adversite output to IPC!"),
                Ok(destroyed) => assert!(destroyed),
            }
        }

        wset.refresh();
        wset.arrange();
    }

    /// Arrange the output workspaces, layer shells, and inform IPC about changes.
    ///
    /// You are expected to call this after you applied your changes to the output, like changing
    /// the current mode, mapping a layer shell, etc.
    pub fn output_resized(&mut self, output: &Output) {
        self.wset_mut_for(output).arrange();
        layer_map_for_output(output).arrange();

        let geometry = output.geometry();
        let refresh_rate = output.current_mode().unwrap().refresh as f32 / 1_000.0;
        let scale = output.current_scale();
        let (int_scale, frac_scale) = (scale.integer_scale(), scale.fractional_scale());
        {
            let path = format!(
                "/fht/desktop/Compositor/Output/{}",
                output.name().replace("-", "_")
            );
            async_std::task::block_on(async {
                let iface_ref = DBUS_CONNECTION
                    .object_server()
                    .interface::<_, IpcOutput>(path.as_str())
                    .unwrap();
                let mut iface = iface_ref.get_mut();

                if iface.location != (geometry.loc.x, geometry.loc.y) {
                    iface.location = (geometry.loc.x, geometry.loc.y);
                    iface
                        .location_changed(iface_ref.signal_context())
                        .await
                        .unwrap();
                }

                if iface.size != (geometry.size.w, geometry.size.h) {
                    iface.size = (geometry.size.w, geometry.size.h);
                    iface
                        .size_changed(iface_ref.signal_context())
                        .await
                        .unwrap();
                }

                if iface.refresh_rate != refresh_rate {
                    iface.refresh_rate = refresh_rate;
                    iface
                        .refresh_rate_changed(iface_ref.signal_context())
                        .await
                        .unwrap();
                }

                if iface.integer_scale != int_scale {
                    iface.integer_scale = int_scale;
                    iface
                        .integer_scale_changed(iface_ref.signal_context())
                        .await
                        .unwrap();
                }

                if iface.fractional_scale != frac_scale {
                    iface.fractional_scale = frac_scale;
                    iface
                        .fractional_scale_changed(iface_ref.signal_context())
                        .await
                        .unwrap();
                }
            });
        }
    }

    /// Get the active output, generally the one with the cursor on it, fallbacking to the first
    /// available output.
    pub fn active_output(&self) -> Output {
        self.focus_state
            .output
            .clone()
            .unwrap_or_else(|| self.outputs().next().unwrap().clone())
    }

    /// Get the output with this name, if any.
    pub fn output_named(&self, name: &str) -> Option<Output> {
        if name == "active" {
            Some(self.active_output())
        } else {
            self.outputs().find(|o| &o.name() == name).cloned()
        }
    }

    /// List all the outputs and a reference to their associated workspace set.
    pub fn workspaces(&self) -> impl Iterator<Item = (&Output, &WorkspaceSet)> {
        self.workspaces.iter()
    }

    /// List all the outptuts and a mutable reference to their associated workspace set.
    pub fn workspaces_mut(&mut self) -> impl Iterator<Item = (&Output, &mut WorkspaceSet)> {
        self.workspaces.iter_mut()
    }

    /// Get a reference to the workspace set associated with this output
    ///
    /// ## PANICS
    ///
    /// This function panics if you didn't register the output.
    pub fn wset_for(&self, output: &Output) -> &WorkspaceSet {
        self.workspaces
            .get(output)
            .expect("Tried to get the WorkspaceSet of a non-existing output!")
    }

    /// Get a mutable reference to the workspace set associated with this output
    ///
    /// ## PANICS
    ///
    /// This function panics if you didn't register the output.
    pub fn wset_mut_for(&mut self, output: &Output) -> &mut WorkspaceSet {
        self.workspaces
            .get_mut(output)
            .expect("Tried to get the WorkspaceSet of a non-existing output!")
    }
}

impl Fht {
    /// Send frame events to [`WlSurface`]s after submitting damage to the backend buffer.
    ///
    /// This function handles primary scanout outputs (so that [`WlSurface`]s send frames
    /// immediatly to a specific render surface, the one in [`RenderElementStates`])
    ///
    /// This function also, if you provide one, sends render and scanout feedbacks to
    /// [`WlSurface`]s
    #[profiling::function]
    pub fn send_frames(
        &self,
        output: &Output,
        render_element_states: &RenderElementStates,
        dmabuf_feedback: Option<SurfaceDmabufFeedback>,
    ) {
        let time = self.clock.now();
        let throttle = Some(std::time::Duration::from_secs(1));

        for window in self.visible_windows_for_output(output) {
            window.with_surfaces(|surface, states| {
                let primary_scanout_output = update_surface_primary_scanout_output(
                    surface,
                    output,
                    states,
                    render_element_states,
                    default_primary_scanout_output_compare,
                );
                if let Some(output) = primary_scanout_output {
                    with_fractional_scale(states, |fractional_scale| {
                        fractional_scale
                            .set_preferred_scale(output.current_scale().fractional_scale())
                    });
                }
            });
            window.send_frame(output, time, throttle, surface_primary_scanout_output);
            if let Some(ref dmabuf_feedback) = dmabuf_feedback {
                window.send_dmabuf_feedback(output, surface_primary_scanout_output, |surface, _| {
                    select_dmabuf_feedback(
                        surface,
                        render_element_states,
                        &dmabuf_feedback.render_feedback,
                        &dmabuf_feedback.scanout_feedback,
                    )
                })
            }
        }

        let map = layer_map_for_output(output);
        for layer_surface in map.layers() {
            layer_surface.with_surfaces(|surface, states| {
                let primary_scanout_output = update_surface_primary_scanout_output(
                    surface,
                    output,
                    states,
                    render_element_states,
                    default_primary_scanout_output_compare,
                );

                if let Some(output) = primary_scanout_output {
                    with_fractional_scale(states, |fraction_scale| {
                        fraction_scale
                            .set_preferred_scale(output.current_scale().fractional_scale());
                    });
                }
            });

            layer_surface.send_frame(output, time, throttle, surface_primary_scanout_output);
            if let Some(ref dmabuf_feedback) = dmabuf_feedback {
                layer_surface.send_dmabuf_feedback(
                    output,
                    surface_primary_scanout_output,
                    |surface, _| {
                        select_dmabuf_feedback(
                            surface,
                            render_element_states,
                            &dmabuf_feedback.render_feedback,
                            &dmabuf_feedback.scanout_feedback,
                        )
                    },
                );
            }
        }
    }

    /// Take the presentation feedback of every visible [`WlSurface`] on this output.
    #[profiling::function]
    pub fn take_presentation_feedback(
        &self,
        output: &Output,
        render_element_states: &RenderElementStates,
    ) -> OutputPresentationFeedback {
        let mut output_presentation_feedback = OutputPresentationFeedback::new(output);

        for window in &self.wset_for(output).active().windows {
            window.take_presentation_feedback(
                &mut output_presentation_feedback,
                surface_primary_scanout_output,
                |surface, _| {
                    surface_presentation_feedback_flags_from_states(surface, render_element_states)
                },
            )
        }

        let map = layer_map_for_output(output);
        for layer_surface in map.layers() {
            layer_surface.take_presentation_feedback(
                &mut output_presentation_feedback,
                surface_primary_scanout_output,
                |surface, _| {
                    surface_presentation_feedback_flags_from_states(surface, render_element_states)
                },
            );
        }

        output_presentation_feedback
    }
}

#[derive(Debug, Clone)]
pub struct SurfaceDmabufFeedback {
    pub render_feedback: DmabufFeedback,
    pub scanout_feedback: DmabufFeedback,
}

#[derive(Default, Debug)]
pub struct ClientState {
    /// Per-client state of wl_compositor.
    pub compositor: CompositorClientState,
    /// wl_security_context state.
    pub security_context: Option<SecurityContext>,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: smithay::reexports::wayland_server::backend::ClientId) {}
    fn disconnected(
        &self,
        _client_id: smithay::reexports::wayland_server::backend::ClientId,
        _reason: smithay::reexports::wayland_server::backend::DisconnectReason,
    ) {
    }
}

/// Retrieve the [`EguiState`] for a given [`Output`]
///
/// If none existed before a new [`EguiState`] will be created for this output
pub fn egui_state_for_output(output: &Output) -> Rc<EguiState> {
    output
        .user_data()
        .get_or_insert(|| Rc::new(EguiState::new(output.geometry().size.as_logical())))
        .clone()
}

#[derive(Default, Debug)]
pub struct FocusState {
    pub output: Option<Output>,
    pub focus_target: Option<KeyboardFocusTarget>,
}
