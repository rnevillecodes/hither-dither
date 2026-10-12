// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // `--vst3-scan <bundle>` runs the plugin scanner instead of the app.
    if let Some(code) = hither_dither_lib::plugins::scan::child_main() {
        std::process::exit(code);
    }
    hither_dither_lib::run()
}
