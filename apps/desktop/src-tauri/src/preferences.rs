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

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LegacyPreviewMode {
    #[default]
    Economy,
    Motion,
    Native,
    Manual,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum StreamMode {
    #[default]
    Snapshot,
    Native,
    Manual,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum PreviewPlayback {
    #[default]
    Automatic,
    Manual,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
struct PreviewSettings {
    playback: PreviewPlayback,
    display: String,
    max_edge: u32,
    interval_ms: u32,
}

impl Default for PreviewSettings {
    fn default() -> Self {
        Self {
            playback: PreviewPlayback::Automatic,
            display: String::new(),
            max_edge: 960,
            interval_ms: 2_000,
        }
    }
}

impl PreviewSettings {
    fn normalize(&mut self) {
        self.max_edge = clamp_stream_edge(self.max_edge);
        self.interval_ms = self.interval_ms.clamp(250, 10_000);
    }
}

#[derive(Clone, Serialize)]
struct StreamSettings {
    mode: StreamMode,
    display: String,
    max_edge: u32,
    fps: u32,
}

impl Default for StreamSettings {
    fn default() -> Self {
        Self {
            mode: StreamMode::Snapshot,
            display: String::new(),
            max_edge: 1280,
            fps: 12,
        }
    }
}

impl StreamSettings {
    fn normalize(&mut self) {
        self.max_edge = clamp_stream_edge(self.max_edge);
        self.fps = self.fps.clamp(1, 60);
    }
}

pub fn clamp_stream_edge(edge: u32) -> u32 {
    edge.clamp(320, 3840)
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(from = "SavedPreferences")]
pub struct Preferences {
    close_behavior: CloseBehavior,
    preview: PreviewSettings,
    stream: StreamSettings,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct SavedPreferences {
    close_behavior: CloseBehavior,
    preview: Option<PreviewSettings>,
    stream: Option<SavedStreamSettings>,
    preview_mode: LegacyPreviewMode,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct SavedStreamSettings {
    mode: StreamMode,
    display: String,
    max_edge: Option<u32>,
    fps: Option<u32>,
    native_edge: Option<u32>,
    native_fps: Option<u32>,
    snapshot_edge: Option<u32>,
    snapshot_interval_ms: Option<u32>,
}

impl From<SavedPreferences> for Preferences {
    fn from(saved: SavedPreferences) -> Self {
        let stream = saved.stream.unwrap_or_else(|| {
            let mut stream = SavedStreamSettings::default();
            match saved.preview_mode {
                LegacyPreviewMode::Economy => {}
                LegacyPreviewMode::Motion => {
                    stream.snapshot_edge = Some(960);
                    stream.snapshot_interval_ms = Some(200);
                }
                LegacyPreviewMode::Native => stream.mode = StreamMode::Native,
                LegacyPreviewMode::Manual => stream.mode = StreamMode::Manual,
            }
            stream
        });
        let mut preview = saved.preview.unwrap_or_else(|| PreviewSettings {
            playback: if matches!(stream.mode, StreamMode::Manual) {
                PreviewPlayback::Manual
            } else {
                PreviewPlayback::Automatic
            },
            display: stream.display.clone(),
            ..PreviewSettings::default()
        });
        let mut stream = StreamSettings {
            mode: match stream.mode {
                StreamMode::Manual => StreamMode::Snapshot,
                mode => mode,
            },
            display: stream.display,
            max_edge: stream
                .max_edge
                .or(if matches!(stream.mode, StreamMode::Native) {
                    stream.native_edge
                } else {
                    stream.snapshot_edge
                })
                .unwrap_or_else(|| StreamSettings::default().max_edge),
            fps: stream
                .fps
                .or(if matches!(stream.mode, StreamMode::Native) {
                    stream.native_fps
                } else {
                    stream.snapshot_interval_ms.map(|interval| {
                        let interval = interval.clamp(100, 10_000);
                        (1000 + interval / 2) / interval
                    })
                })
                .unwrap_or_else(|| StreamSettings::default().fps),
        };
        stream.normalize();
        preview.normalize();
        Self {
            close_behavior: saved.close_behavior,
            preview,
            stream,
        }
    }
}

#[derive(Deserialize)]
pub struct PreferenceChanges {
    close_behavior: Option<CloseBehavior>,
    preview: Option<PreviewChanges>,
    stream: Option<StreamChanges>,
}

#[derive(Deserialize)]
struct PreviewChanges {
    playback: Option<PreviewPlayback>,
    display: Option<String>,
    max_edge: Option<u32>,
    interval_ms: Option<u32>,
}

#[derive(Deserialize)]
struct StreamChanges {
    mode: Option<StreamMode>,
    display: Option<String>,
    max_edge: Option<u32>,
    fps: Option<u32>,
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
        self.value
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
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
    changes: PreferenceChanges,
) -> Result<Preferences, String> {
    let mut value = store.value.lock().map_err(|err| err.to_string())?;
    let mut next = value.clone();
    next.close_behavior = changes.close_behavior.unwrap_or(next.close_behavior);
    if let Some(changes) = changes.preview {
        let preview = &mut next.preview;
        preview.playback = changes.playback.unwrap_or(preview.playback);
        if let Some(display) = changes.display {
            preview.display = display;
        }
        preview.max_edge = changes.max_edge.unwrap_or(preview.max_edge);
        preview.interval_ms = changes.interval_ms.unwrap_or(preview.interval_ms);
        preview.normalize();
    }
    let stream_changed = changes.stream.is_some();
    if let Some(changes) = changes.stream {
        let stream = &mut next.stream;
        stream.mode = match changes.mode.unwrap_or(stream.mode) {
            StreamMode::Manual => StreamMode::Snapshot,
            mode => mode,
        };
        if let Some(display) = changes.display {
            stream.display = display;
        }
        stream.max_edge = changes.max_edge.unwrap_or(stream.max_edge);
        stream.fps = changes.fps.unwrap_or(stream.fps);
        stream.normalize();
    }
    let bytes = serde_json::to_vec_pretty(&next).map_err(|err| err.to_string())?;
    let temporary = store.path.with_extension("json.tmp");
    if let Some(parent) = store.path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    std::fs::write(&temporary, bytes).map_err(|err| err.to_string())?;
    std::fs::rename(&temporary, &store.path).map_err(|err| err.to_string())?;
    *value = next.clone();
    drop(value);
    if stream_changed {
        let stream = &next.stream;
        log::debug!(
            "Stream settings saved: {:?}, {} px at up to {} fps",
            stream.mode,
            stream.max_edge,
            stream.fps,
        );
    }
    let _ = app.emit("veil://preferences", ());
    Ok(next)
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

pub fn verify_preferences(store: &PreferenceStore) -> Result<String, String> {
    let current = store.value.lock().map_err(|err| err.to_string())?;
    let bytes = serde_json::to_vec(&*current).map_err(|err| err.to_string())?;
    let saved = match std::fs::read(&store.path) {
        Ok(data) => data,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => bytes.clone(),
        Err(err) => return Err(err.to_string()),
    };
    let decoded: Preferences = serde_json::from_slice(&saved)
        .map_err(|err| format!("Saved settings could not be read: {err}"))?;
    if serde_json::to_vec(&decoded).map_err(|err| err.to_string())? != bytes {
        return Err("Saved settings differ from the running app preferences.".into());
    }
    Ok(format!(
        "Read back {:?} preview at {} px. Stream output: {:?}, {} px at up to {} fps.",
        decoded.preview.playback,
        decoded.preview.max_edge,
        decoded.stream.mode,
        decoded.stream.max_edge,
        decoded.stream.fps,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_playback_settings_keep_their_quality_and_close_behavior() {
        let old: Preferences =
            serde_json::from_str(r#"{"close_behavior":"minimize","preview_mode":"motion"}"#)
                .unwrap();
        assert!(matches!(old.close_behavior, CloseBehavior::Minimize));
        assert!(matches!(old.stream.mode, StreamMode::Snapshot));
        assert_eq!((old.stream.max_edge, old.stream.fps), (960, 5));
        let serialized = serde_json::to_string(&old).unwrap();
        let reopened: Preferences = serde_json::from_str(&serialized).unwrap();
        assert!(matches!(reopened.stream.mode, StreamMode::Snapshot));
        assert_eq!((reopened.stream.max_edge, reopened.stream.fps), (960, 5));
    }

    #[test]
    fn manual_playback_migrates_to_preview_without_turning_stream_output_manual() {
        let old: Preferences = serde_json::from_str(
            r#"{"stream":{"mode":"manual","display":"display-a","snapshot_edge":2560}}"#,
        )
        .unwrap();
        assert!(matches!(old.preview.playback, PreviewPlayback::Manual));
        assert_eq!(old.preview.max_edge, 960);
        assert_eq!(old.preview.display, "display-a");
        assert!(matches!(old.stream.mode, StreamMode::Snapshot));
        assert_eq!(old.stream.max_edge, 2560);
    }
}
