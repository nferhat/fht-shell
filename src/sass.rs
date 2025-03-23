use std::ffi::OsStr;
use std::fs;

use anyhow::Context;

/// This funtion loads the sass contents from `path` and compiles it.
/// Returns the final CSS string.
//
// FIXME: Tracing logging
pub fn load_css_from_path(path: &std::path::Path) -> anyhow::Result<String> {
    let file_contents = fs::read_to_string(path)?;

    match path.extension().and_then(OsStr::to_str) {
        Some("css") => {
            info!(?path, "Loading CSS");
            return Ok(file_contents);
        }
        Some("sass" | "scss") => {
            info!(?path, "Loading SCSS");
            let options = grass::Options::default().load_path(path);
            grass::from_string(&file_contents, &options).context("Failed to parse SCSS")
        }
        _ => {
            info!(?path, "Unknown extension, assuming SCSS");
            let options = grass::Options::default().load_path(path);
            grass::from_string(&file_contents, &options).context("Failed to parse SCSS")
        }
    }
}
