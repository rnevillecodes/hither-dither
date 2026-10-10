pub mod commands;
pub mod peaks;
pub mod state;
pub mod stats;
pub mod wav;

use std::sync::Arc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage::<commands::SharedState>(Arc::new(state::AppState::default()))
        .invoke_handler(tauri::generate_handler![
            commands::open_file,
            commands::get_metadata,
            commands::get_peaks,
            commands::compute_stats,
            commands::get_clip_runs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
