use tauri::State;

use crate::app::AppState;
use crate::features::plugin::config::PluginConfig;
use crate::infra::config::resolve_plugin_dir;
use crate::features::plugin::types::Track;
use crate::features::plugin::PluginState;

async fn persist(state: &AppState, config: &PluginConfig) -> Result<(), String> {
    let value = serde_json::to_value(config).map_err(|e| e.to_string())?;
    state
        .db
        .update_setting("plugin_config", &value)
        .await
        .map_err(|e| e.to_string())
}

fn manager(state: &AppState) -> Result<&crate::features::plugin::PluginManager, String> {
    state
        .plugins
        .as_ref()
        .map(|plugins| plugins.as_ref())
        .ok_or_else(|| "plugin system not available".to_string())
}

#[tauri::command]
pub async fn get_plugin_state(state: State<'_, AppState>) -> Result<PluginState, String> {
    Ok(manager(&state)?.state().await)
}

/// Rescan the plugin directory, hot-loading new/removed plugins.
#[tauri::command]
pub async fn reload_plugins(state: State<'_, AppState>) -> Result<PluginState, String> {
    let plugins = manager(&state)?;
    let configured = {
        let config = state.config.lock().await;
        config.plugin_dir.clone()
    };
    let plugin_dir = resolve_plugin_dir(&configured);
    if let Err(error) = std::fs::create_dir_all(&plugin_dir) {
        tracing::warn!("cannot create plugin dir {:?}: {error}", plugin_dir);
    }
    plugins.reload_from(plugin_dir).await?;
    Ok(plugins.state().await)
}

/// Parse a full song/playlist URL through the playlist-capable providers.
/// A single-song URL returns one Track; a playlist URL returns all entries.
#[tauri::command]
pub async fn parse_online_url(
    state: State<'_, AppState>,
    url: String,
) -> Result<Vec<Track>, String> {
    manager(&state)?.playlist(&url).await
}

#[tauri::command]
pub async fn set_plugin_enabled(
    state: State<'_, AppState>,
    name: String,
    enabled: bool,
) -> Result<PluginState, String> {
    let plugins = manager(&state)?;
    let config = plugins.set_plugin_enabled(&name, enabled).await?;
    persist(&state, &config).await?;
    Ok(plugins.state().await)
}

#[tauri::command]
pub async fn set_plugin_argv(
    state: State<'_, AppState>,
    name: String,
    argv: Vec<String>,
) -> Result<PluginState, String> {
    let plugins = manager(&state)?;
    let config = plugins.set_plugin_argv(&name, argv).await?;
    persist(&state, &config).await?;
    Ok(plugins.state().await)
}

#[tauri::command]
pub async fn set_plugin_platform_argv(
    state: State<'_, AppState>,
    name: String,
    platform: String,
    argv: Vec<String>,
) -> Result<PluginState, String> {
    let plugins = manager(&state)?;
    let config = plugins
        .set_platform_argv(&name, &platform, argv)
        .await?;
    persist(&state, &config).await?;
    Ok(plugins.state().await)
}

#[tauri::command]
pub async fn set_plugin_route_argv(
    state: State<'_, AppState>,
    name: String,
    platform: String,
    capability: String,
    argv: Vec<String>,
) -> Result<PluginState, String> {
    let plugins = manager(&state)?;
    let config = plugins
        .set_route_argv(&name, &platform, &capability, argv)
        .await?;
    persist(&state, &config).await?;
    Ok(plugins.state().await)
}

#[tauri::command]
pub async fn set_plugin_concurrency(
    state: State<'_, AppState>,
    name: String,
    concurrency: Option<u32>,
) -> Result<PluginState, String> {
    let plugins = manager(&state)?;
    let config = plugins
        .set_plugin_concurrency(&name, concurrency)
        .await?;
    persist(&state, &config).await?;
    Ok(plugins.state().await)
}

#[tauri::command]
pub async fn set_plugin_route(
    state: State<'_, AppState>,
    platform: String,
    capability: String,
    providers: Vec<String>,
) -> Result<PluginState, String> {
    let plugins = manager(&state)?;
    let config = plugins
        .set_route_providers(&platform, &capability, providers)
        .await?;
    persist(&state, &config).await?;
    Ok(plugins.state().await)
}
