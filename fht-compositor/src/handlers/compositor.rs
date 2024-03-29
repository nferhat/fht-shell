use std::collections::hash_map::Entry;
use std::sync::Mutex;

use smithay::backend::renderer::utils::{on_commit_buffer_handler, with_renderer_surface_state};
use smithay::delegate_compositor;
use smithay::desktop::{layer_map_for_output, PopupKind};
use smithay::reexports::calloop::Interest;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::Resource;
use smithay::wayland::compositor::{
    add_blocker, add_pre_commit_hook, get_parent, is_sync_subsurface, with_states,
    BufferAssignment, CompositorHandler, SurfaceAttributes,
};
use smithay::wayland::dmabuf::get_dmabuf;
use smithay::wayland::shell::wlr_layer::LayerSurfaceAttributes;
use smithay::wayland::shell::xdg::{
    ToplevelSurface, XdgPopupSurfaceRoleAttributes, XdgToplevelSurfaceRoleAttributes,
};

use crate::shell::WindowMapSettingsInternal;
use crate::state::{Fht, State};

/// Ensures that the [`WlSurface`] has a render buffer
fn has_render_buffer(surface: &WlSurface) -> bool {
    // If there's no renderer surface data, just assume the surface didn't even get recognized by
    // the renderer
    with_renderer_surface_state(surface, |s| s.buffer().is_some()).unwrap_or(false)
}

/// Returns whether this [`Toplevel`] initial configure was sent.
fn initial_configure_sent(toplevel: &ToplevelSurface) -> bool {
    with_states(toplevel.wl_surface(), |states| {
        states
            .data_map
            .get::<Mutex<XdgToplevelSurfaceRoleAttributes>>()
            .unwrap()
            .lock()
            .unwrap()
            .initial_configure_sent
    })
}

impl State {
    /// Process a commit request for a root surface.
    fn maybe_map_pending_window(surface: &WlSurface, state: &mut Fht) {
        // First check: the pending window may be a pending one, needing both an initial configure
        // call and a prepapring before mapping.
        let Entry::Occupied(entry) = state.pending_windows.entry(surface.clone()) else {
            return;
        };

        let window = entry.get().clone();
        window.0.on_commit();

        // We don't have a render buffer, send initial configure to window so it acknowledges it
        // needs one and send additional data with it.
        if !has_render_buffer(surface)
            || window
                .user_data()
                .get::<WindowMapSettingsInternal>()
                .is_none()
        {
            state.loop_handle.insert_idle(move |state| {
                if let Some(toplevel) = window.0.toplevel().filter(|t| t.alive()) {
                    state.fht.prepare_map_window(&window);
                    if !initial_configure_sent(toplevel) {
                        toplevel.send_configure();
                    } else {
                        toplevel.send_pending_configure();
                    }
                }
            });

            return;
        }

        // We are now prepared, map as normal.
        let window = entry.remove();
        state.map_window(window);
    }

    /// Process a potential commit request for a layer shell
    fn maybe_map_pending_layer_shell(surface: &WlSurface, state: &mut Fht) {
        let Entry::Occupied(entry) = state.pending_layers.entry(surface.clone()) else {
            return;
        };

        // Goofy process but we need it before
        let (layer_surface, _) = entry.get();
        let initial_configure_sent = with_states(layer_surface.wl_surface(), |states| {
            states
                .data_map
                .get::<Mutex<LayerSurfaceAttributes>>()
                .unwrap()
                .lock()
                .unwrap()
                .initial_configure_sent
        });
        if !initial_configure_sent {
            layer_surface.layer_surface().send_configure();
            return;
        }

        let (layer_surface, output) = entry.remove();
        if let Err(err) = layer_map_for_output(&output).map_layer(&layer_surface) {
            warn!(?err, "Failed to map layer surface!");
        };
        state.output_resized(&output);
    }
}

/// Ensures that the initial configure event is sent for a popup.
fn popup_ensure_initial_configure(popup: &PopupKind) {
    let PopupKind::Xdg(ref popup) = popup else {
        return;
    };

    let initial_configure_sent = with_states(popup.wl_surface(), |states| {
        states
            .data_map
            .get::<Mutex<XdgPopupSurfaceRoleAttributes>>()
            .unwrap()
            .lock()
            .unwrap()
            .initial_configure_sent
    });
    if !initial_configure_sent {
        // NOTE: A popup initial configure should never fail
        popup.send_configure().expect("Initial configure failed!");
    }
}

impl CompositorHandler for State {
    fn compositor_state(&mut self) -> &mut smithay::wayland::compositor::CompositorState {
        &mut self.fht.compositor_state
    }

    fn client_compositor_state<'a>(
        &self,
        client: &'a smithay::reexports::wayland_server::Client,
    ) -> &'a smithay::wayland::compositor::CompositorClientState {
        &client
            .get_data::<crate::state::ClientState>()
            .unwrap()
            .compositor
    }

    fn new_surface(&mut self, surface: &WlSurface) {
        add_pre_commit_hook::<Self, _>(surface, move |state, _dh, surface| {
            let maybe_dmabuf = with_states(surface, |surface_data| {
                surface_data
                    .cached_state
                    .pending::<SurfaceAttributes>()
                    .buffer
                    .as_ref()
                    .and_then(|assignment| match assignment {
                        BufferAssignment::NewBuffer(buffer) => get_dmabuf(buffer).ok(),
                        _ => None,
                    })
            });
            if let Some(dmabuf) = maybe_dmabuf {
                if let Ok((blocker, source)) = dmabuf.generate_blocker(Interest::READ) {
                    let client = surface.client().unwrap();
                    let res = state
                        .fht
                        .loop_handle
                        .insert_source(source, move |_, _, state| {
                            let dh = state.fht.display_handle.clone();
                            state
                                .client_compositor_state(&client)
                                .blocker_cleared(state, &dh);
                            Ok(())
                        });
                    if res.is_ok() {
                        add_blocker(surface, blocker);
                    }
                }
            }
        });
    }

    #[profiling::function]
    fn commit(&mut self, surface: &WlSurface) {
        // Allow smithay to manage internally wl_surfaces and wl_buffers
        //
        // Have to call this at the top of here before handling anything otherwise it'll mess
        // buffer management
        on_commit_buffer_handler::<Self>(surface);
        #[cfg(feature = "udev_backend")]
        if let crate::backend::Backend::Udev(ref mut data) = &mut self.backend {
            data.early_import(surface);
        }

        // We are already synced, why bother going additional computations
        if is_sync_subsurface(surface) {
            return;
        }

        let mut root_surface = surface.clone();
        while let Some(new_parent) = get_parent(&root_surface) {
            root_surface = new_parent;
        }

        if surface == &root_surface {
            State::maybe_map_pending_window(&surface, &mut self.fht);
            State::maybe_map_pending_layer_shell(&surface, &mut self.fht);
        }

        // Maybe commiting a mapped window.
        if let Some(window) = self.fht.find_window(surface).filter(|w| w.is_wayland()) {
            window.0.on_commit();
        }

        // Or maybe a popup/subsurface
        if let Some(popup) = self.fht.popups.find_popup(surface) {
            popup_ensure_initial_configure(&popup);
        }
        self.fht.popups.commit(surface);

        // Try to redraw the output
        if let Some(output) = self.fht.visible_output_for_surface(surface) {
            self.backend
                .schedule_render_output(output, &self.fht.loop_handle);
        }
    }
}

delegate_compositor!(State);
