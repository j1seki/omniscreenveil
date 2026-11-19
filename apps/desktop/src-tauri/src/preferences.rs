use std::{path::PathBuf, sync::Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseBehavior {
    #[default]
    Quit,
    Minimize,
}

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewMode {
    #[default]
    Economy,
    Motion,
    Manual,
}

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Preferences {
    pub close_behavior: CloseBehavior,
    pub preview_mode: PreviewMode,
}

pub struct PreferenceStore {
    path: PathBuf,
    value: Mutex<Preferences>,
}

impl PreferenceStore {
    pub fn new(path: PathBuf) -> Self {
        let value = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            path,
            value: Mutex::new(value),
        }
    }

    pub fn get(&self) -> Preferences {
        *self.value.lock().unwrap_or_else(|err| err.into_inner())
    }
}

#[tauri::command]
pub fn get_preferences(store: State<'_, PreferenceStore>) -> Preferences {
    store.get()
}

#[tauri::command]
pub fn save_preferences(
    app: AppHandle,
    store: State<'_, PreferenceStore>,
    preferences: Preferences,
) -> Result<(), String> {
    let mut value = store.value.lock().map_err(|err| err.to_string())?;
    let bytes = serde_json::to_vec_pretty(&preferences).map_err(|err| err.to_string())?;
    let temporary = store.path.with_extension("json.tmp");
    if let Some(parent) = store.path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    std::fs::write(&temporary, bytes).map_err(|err| err.to_string())?;
    std::fs::rename(&temporary, &store.path).map_err(|err| err.to_string())?;
    *value = preferences;
    drop(value);
    app.emit("veil://preferences", preferences)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn login_startup(app: AppHandle, enabled: Option<bool>) -> Result<bool, String> {
    let manager = app.autolaunch();
    match enabled {
        Some(true) => manager.enable().map_err(|err| err.to_string())?,
        Some(false) => manager.disable().map_err(|err| err.to_string())?,
        None => {}
    }
    manager.is_enabled().map_err(|err| err.to_string())
}

pub fn minimize_on_close(app: &AppHandle) -> bool {
    app.try_state::<PreferenceStore>()
        .is_some_and(|store| matches!(store.get().close_behavior, CloseBehavior::Minimize))
}
