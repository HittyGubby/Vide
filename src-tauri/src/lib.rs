mod models;
mod infra;
mod features;
mod app;

pub use models::*;
pub use infra::*;

use std::path::PathBuf;

fn db_path() -> PathBuf {
    let mut path = dirs::data_dir().unwrap_or_else(|| {
        // Fallback: use the current working directory
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    });
    path.push("vide");
    std::fs::create_dir_all(&path).ok();
    path.push("vide.db");
    path
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let app_state = {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
        rt.block_on(async {
            let db_path = db_path();
            tracing::info!("Using database at: {}", db_path.display());
            app::AppState::initialize(&db_path).await
        })
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            // Library
            features::ui::list_songs,
            features::ui::get_song,
            features::ui::search_songs,
            features::ui::scan_library,
            // Playback
            features::ui::get_player_status,
            features::ui::play_song,
            features::ui::stop_playback,
            // Search
            features::ui::fuzzy_search_songs,
            features::ui::autocomplete,
            // Tags
            features::ui::get_meta_tags,
            features::ui::add_tag,
            features::ui::remove_tag,
            features::ui::get_song_tags,
            features::ui::get_namespaces,
            // Playlists
            features::ui::import_m3u,
            features::ui::get_playlist,
            features::ui::export_playlist,
            // Server
            features::ui::start_server,
            features::ui::stop_server,
            features::ui::get_server_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
