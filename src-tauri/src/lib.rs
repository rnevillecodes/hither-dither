pub mod commands;
pub mod edit;
pub mod peaks;
pub mod playback;
pub mod plugins;
pub mod state;
pub mod stats;
pub mod wav;

use std::sync::Arc;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    plugins::main_thread::mark_main_thread();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let plugins = commands::open_plugin_cache(app.path().app_data_dir()?)?;
            app.manage(plugins.clone());
            commands::start_plugin_scan(app.handle(), &plugins, false);
            Ok(())
        })
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
            commands::list_plugins,
            commands::plugin_folders,
            commands::set_plugin_folders,
            commands::rescan_plugins,
            commands::set_plugin_enabled,
            commands::take_startup_notices,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
