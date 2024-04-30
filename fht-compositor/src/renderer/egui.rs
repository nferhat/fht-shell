//! Helper functions to build the egui debug overlay.

use smithay::output::Output;
use smithay_egui::{egui, egui_extras};

use crate::shell::workspaces::WorkspaceSwitchAnimation;
use crate::state::Fht;
use crate::utils::fps::Fps;
use crate::utils::geometry::{PointExt, PointGlobalExt};
use crate::utils::output::OutputExt;

#[profiling::function]
pub fn egui_debug_overlay(context: &egui::Context, output: &Output, state: &Fht, fps: &mut Fps) {
    let area = egui::Window::new(output.name())
        .resizable(false)
        .collapsible(false)
        .movable(true);
    let mode = output.current_mode().unwrap();
    let scale = output.current_scale().fractional_scale();
    let pointer_loc = state
        .pointer
        .current_location()
        .as_global()
        .to_local(output);
    let geo = output.geometry();
    let wset = state.wset_for(output);

    let active_idx_str = if let Some(WorkspaceSwitchAnimation { ref target_idx, .. }) =
        wset.switch_animation.as_ref()
    {
        format!(
            "{active_idx} => {target_idx}",
            active_idx = wset.get_active_idx()
        )
    } else {
        wset.get_active_idx().to_string()
    };

    let (max_frametime, min_frametime, avg_frametime, avg_fps) = (
        fps.max_frametime().as_secs_f64() * 1_000.0,
        fps.min_frametime().as_secs_f64() * 1_000.0,
        fps.avg_frametime().as_secs_f64() * 1_000.0,
        fps.avg_fps(),
    );
    let avg_rendertime = fps.avg_rendertime(5).as_secs_f64();

    let format_info = |ui: &mut egui::Ui, name, data| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.label(format!("{name}: "));
            ui.code(data);
        });
    };

    area.show(context, |ui| {
        ui.collapsing("Framerate information", |ui| {
            format_info(ui, "FPS", format!("{:0>07.3}", avg_fps));
            format_info(
                ui,
                "Average rendertime",
                format!("{:0>07.3}", avg_rendertime),
            );
            format_info(ui, "Minimum frametime", format!("{:0>07.3}", min_frametime));
            format_info(ui, "Average frametime", format!("{:0>07.3}", avg_frametime));
            format_info(ui, "Maximum frametime", format!("{:0>07.3}", max_frametime));
        });

        ui.collapsing("Mode information", |ui| {
            format_info(ui, "Refresh rate", format!("{}", mode.refresh / 1_000));
            format_info(
                ui,
                "Size in pixels",
                format!("{}x{}", mode.size.w, mode.size.h),
            );
            format_info(
                ui,
                "Current location",
                format!("({}, {})", geo.loc.x, geo.loc.y),
            );
            format_info(ui, "Current scale", format!("{:0>04.2}", scale))
        });

        ui.collapsing("Misc information", |ui| {
            format_info(
                ui,
                "Pointer location",
                format!("({:0>09.4}, {:0>09.4})", pointer_loc.x, pointer_loc.y),
            );
            format_info(ui, "Active workspace idx", active_idx_str);
            format_info(ui, "Animations ongoing", format!("Figure this out"));
        });
    });
}

#[profiling::function]
pub fn egui_config_error(context: &egui::Context, error: &anyhow::Error) {
    let area = egui::Window::new("Failed to reload config!")
        .anchor(egui::Align2::CENTER_TOP, (0.0, 10.0))
        .resizable(false)
        .collapsible(false)
        .movable(true);
    area.show(context, |ui| {
        ui.label(error.to_string());
        ui.label(error.root_cause().to_string());
    });
}

const USEFUL_DEFAULT_KEYBINDS: [(&str, &str); 8] = [
    ("Mod+Return", "Spawn alacritty"),
    ("Mod+P", "Launch `wofi --show drun`"),
    ("Mod+Q", "Exit the compositor"),
    ("Mod+Ctrl+R", "Reload the configuration"),
    ("Mod+J", "Focus the next window"),
    ("Mod+K", "Focus the previous window"),
    ("Mod+1-9", "Focus the nth workspace"),
    (
        "Mod+Shift+1-9",
        "Send the focused window to the nth workspace",
    ),
];

#[profiling::function]
pub fn egui_greeting_message(context: &egui::Context) {
    let area = egui::Window::new("Welcome to fht-compositor").resizable(false);
    area.show(context, |ui| {
        ui.label("If you are seeing this message, that means you successfully installed and ran the compositor with no issues! Congratulations!");

        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.label("The compositor should have now copied a starter configuration to the following path:");
            ui.code("$XDG_CONFIG_HOME/.config/fht/compositor.ron");
        });

        ui.add_space(8.0);
        ui.label("You can disable this message by setting greet to false in your config file!");

        ui.add_space(12.0);
        ui.heading("Warning notice");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.label("Bear in mind that fht-compositor is STILL an alpha-quality software, and breaking changes can and will happen. ");
            ui.label("If you encounter any issues, or want to contribute, feel free to check out the ");
            ui.hyperlink_to("github page.", "https://github.com/nferhat/fht-shell/blob/main/fht-compositor/");
        });

        ui.add_space(12.0);
        ui.label("Some useful keybinds to know that are in this default config:");
        egui_extras::TableBuilder::new(ui)
            .column(egui_extras::Column::exact(100.0))
            .column(egui_extras::Column::remainder())
            .striped(true)
            .header(15.0, |mut header_row| {
                header_row.col(|ui| { ui.label("Key pattern"); });
                header_row.col(|ui| { ui.label("Description"); });
            })
            .body(|mut body| {
                for (key_pattern, description) in USEFUL_DEFAULT_KEYBINDS {
                    body.row(15.0, |mut row| {
                        row.col(|ui| { ui.code(key_pattern); });
                        row.col(|ui| { ui.label(description); });
                    });
                }
            });
    });
}