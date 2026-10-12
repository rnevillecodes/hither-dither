pub mod commands;
pub mod edit;
pub mod peaks;
pub mod playback;
pub mod plugins;
pub mod state;
pub mod stats;
pub mod wav;

use std::sync::Arc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage::<commands::SharedState>(Arc::new(state::AppState::default()))
        .manage::<commands::SharedPlayer>(Arc::new(playback::Player::new()))
        .invoke_handler(tauri::generate_handler![
            commands::open_file,
            commands::cancel_open,
            commands::close_file,
            commands::get_metadata,
            commands::get_peaks,
            commands::compute_stats,
            commands::get_clip_runs,
            commands::playback_info,
            commands::play,
            commands::stop_playback,
            commands::set_volume,
            commands::set_loop,
            commands::apply_edit,
            commands::undo_edit,
            commands::redo_edit,
            commands::save_file,
            commands::set_clip,
            commands::set_crossfade,
            commands::get_clip,
            commands::wait_analysis,
            commands::preview_edits,
            commands::set_play_range,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
