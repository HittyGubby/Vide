mod models;
mod infra;
mod features;
mod app;

pub use models::*;
pub use infra::*;

use std::path::PathBuf;
use tauri::Manager;

fn db_path() -> PathBuf {
    let mut path = dirs::data_dir().unwrap_or_else(|| {
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
        .setup(|app| {
            use tauri::menu::{MenuBuilder, MenuItemBuilder};
            use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

            let toggle = MenuItemBuilder::with_id("toggle", "显示/隐藏").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "退出").build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&toggle)
                .separator()
                .item(&quit)
                .build()?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Vide")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "toggle" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let visible = window.is_visible().unwrap_or(true);
                            if visible {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let visible = window.is_visible().unwrap_or(true);
                            if visible {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Library
            features::ui::list_songs,
            features::ui::list_library_songs,
            features::ui::import_songs,
            features::ui::get_song,
            features::ui::search_songs,
            features::ui::scan_library,
            features::ui::clear_song_cache,
            features::ui::download_song,
            // Config
            features::ui::get_library_roots,
            features::ui::add_library_root,
            features::ui::remove_library_root,
            features::ui::get_exclude_roots,
            features::ui::add_exclude_root,
            features::ui::remove_exclude_root,
            // Playback
            features::ui::get_player_status,
            features::ui::get_lyrics,
            features::ui::play_song,
            features::ui::set_play_queue,
            features::ui::play_next,
            features::ui::play_prev,
            features::ui::set_play_mode,
            features::ui::stop_playback,
            features::ui::toggle_pause,
            // Seek
            features::ui::seek,
            features::ui::set_speed,
            features::ui::set_volume,
            features::ui::set_mute,
            // Online Search
            features::ui::search_online,
            features::ui::resolve_song,
            features::ui::import_playlist_url,
            // Search
            features::ui::fuzzy_search_songs,
            features::ui::autocomplete,
            // Search Presets
            features::ui::list_search_presets,
            features::ui::save_search_preset,
            features::ui::delete_search_preset,
            features::ui::get_last_search_preset,
            features::ui::set_last_search_preset,
            // App Settings
            features::ui::get_app_settings,
            features::ui::set_app_settings,
            features::ui::get_last_played,
            features::ui::save_last_played,
            // Tags
            features::ui::get_meta_tags,
            features::ui::add_tag,
            features::ui::remove_tag,
            features::ui::get_song_tags,
            features::ui::get_namespaces,
            // Playlists
            features::ui::import_m3u,
            features::ui::get_playlist,
            features::ui::list_playlists,
            features::ui::export_playlist,
            // Server
            features::ui::start_server,
            features::ui::stop_server,
            features::ui::get_server_status,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // When the app is about to exit, stop playback so the mpv
            // subprocess is terminated and does not keep playing.
            if let tauri::RunEvent::ExitRequested { .. } = event {
                if let Some(state) = app.try_state::<app::AppState>() {
                    let player = state.player.clone();
                    let handle = tauri::async_runtime::block_on(async move {
                        player.lock().await.stop().await
                    });
                    let _ = handle;
                }
            }
        });
}
