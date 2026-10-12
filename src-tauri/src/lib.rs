pub mod commands;
pub mod edit;
pub mod fx;
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
            let ui = Arc::new(app.handle().clone());
            let state = Arc::new(state::AppState::new(ui));
            // A plugin's new latency is heard at once, off the UI thread.
            let (weak, player) = (
                Arc::downgrade(&state),
                app.state::<commands::SharedPlayer>().inner().clone(),
            );
            state.on_fx_changed(move |file_id| {
                let (weak, player) = (weak.clone(), player.clone());
                std::thread::spawn(move || {
                    if let Some(state) = weak.upgrade() {
                        commands::follow_playing(&state, &player, file_id);
                    }
                });
            });
            app.manage::<commands::SharedState>(state);
            Ok(())
        })
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
            commands::get_chain,
            commands::add_effect,
            commands::remove_effect,
            commands::move_effect,
            commands::set_effect_bypass,
            commands::set_bypass_all,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
