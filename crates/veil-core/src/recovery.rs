//! Best-effort crash recovery for protections with OS-visible side effects.
//!
//! `StrictHide` and `Exclusion` change real OS state. If the UI process is
//! killed (power loss, task manager, crash), that state would otherwise stay
//! applied forever: a hidden window is never remapped, and an excluded window
//! can remain excluded because the injected helper keeps polling the stale
//! window property. The journal is scoped to the portable-aware application
//! config directory, so portable builds use their local profile location.
//!
//! The journal records the intended state before an operation is attempted. On
//! the next start, the app asks the backend to release records whose window
//! still exists with the same PID. Backend restart recovery is best-effort:
//! some platforms may require process-local state and cannot restore every
//! side effect after a crash. `Surface` is deliberately excluded:
//! it is a compositor effect with no state to restore.
//!
//! This is not a security guarantee. If the app is never restarted, recovery
//! never runs.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::state::AppliedProtection;
use crate::{ProtectionMode, WindowId, WindowInfo};

/// Name of the journal file next to `profile.json`.
pub const RECOVERY_FILE: &str = "recovery.json";

#[derive(Debug, Error)]
pub enum RecoveryError {
    #[error("could not read recovery journal at {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not parse recovery journal at {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("could not write recovery journal at {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// One protection that must be undone if this session dies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryRecord {
    pub window: WindowId,
    pub pid: u32,
    pub mode: ProtectionMode,
    pub saved_at_unix: u64,
}

impl RecoveryRecord {
    /// Whether this mode changes OS state that needs restoring.
    pub fn needs_restore(mode: ProtectionMode) -> bool {
        matches!(mode, ProtectionMode::StrictHide | ProtectionMode::Exclusion)
    }
}

/// A small, synchronous JSON journal. Writes are atomic via a temp file.
#[derive(Debug, Clone)]
pub struct RecoveryJournal {
    path: PathBuf,
}

impl RecoveryJournal {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            path: directory.into().join(RECOVERY_FILE),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<Vec<RecoveryRecord>, RecoveryError> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => serde_json::from_str(&text).map_err(|source| RecoveryError::Parse {
                path: self.path.clone(),
                source,
            }),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(source) => Err(RecoveryError::Read {
                path: self.path.clone(),
                source,
            }),
        }
    }

    /// Persist a snapshot. An empty snapshot removes the journal entirely, so a
    /// cleanly released session leaves no work for the next launch.
    pub fn save(&self, records: &[RecoveryRecord]) -> Result<(), RecoveryError> {
        if records.is_empty() {
            return self.clear();
        }
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| RecoveryError::Write {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let json =
            serde_json::to_string_pretty(records).map_err(|source| RecoveryError::Parse {
                path: self.path.clone(),
                source,
            })?;
        let tmp = self
            .path
            .with_extension(format!("{}.tmp", std::process::id()));
        std::fs::write(&tmp, json).map_err(|source| RecoveryError::Write {
            path: tmp.clone(),
            source,
        })?;
        std::fs::rename(&tmp, &self.path).map_err(|source| RecoveryError::Write {
            path: self.path.clone(),
            source,
        })
    }

    pub fn clear(&self) -> Result<(), RecoveryError> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(RecoveryError::Write {
                path: self.path.clone(),
                source,
            }),
        }
    }
}

/// Build a journal snapshot from the engine's applied set.
///
/// Protections with OS side effects are recorded, including failed attempts:
/// backends can fail after a partial side effect, and release is idempotent.
/// The PID comes from live inventory so a reused window handle cannot cause us
/// to release the wrong application's window.
pub fn snapshot(applied: &[AppliedProtection], windows: &[WindowInfo]) -> Vec<RecoveryRecord> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    applied
        .iter()
        .filter(|a| RecoveryRecord::needs_restore(a.mode))
        .filter_map(|a| {
            windows
                .iter()
                .find(|w| w.id == a.window)
                .map(|w| RecoveryRecord {
                    window: a.window,
                    pid: w.pid,
                    mode: a.mode,
                    saved_at_unix: now,
                })
        })
        .collect()
}

/// Build a durable pre-operation journal from the current intended windows.
/// This is deliberately conservative: a crash between journaling and applying
/// causes a harmless release attempt on restart.
pub fn intended_snapshot(windows: &[WindowInfo], mode: ProtectionMode) -> Vec<RecoveryRecord> {
    if !RecoveryRecord::needs_restore(mode) {
        return Vec::new();
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    windows
        .iter()
        .map(|w| RecoveryRecord {
            window: w.id,
            pid: w.pid,
            mode,
            saved_at_unix: now,
        })
        .collect()
}

/// Whether a journal record still refers to the same live window.
pub fn matches_live(record: &RecoveryRecord, windows: &[WindowInfo]) -> bool {
    windows
        .iter()
        .any(|w| w.id == record.window && w.pid == record.pid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;
    use crate::testutil::window;

    #[test]
    fn only_side_effect_modes_are_restorable() {
        assert!(RecoveryRecord::needs_restore(ProtectionMode::StrictHide));
        assert!(RecoveryRecord::needs_restore(ProtectionMode::Exclusion));
        assert!(!RecoveryRecord::needs_restore(ProtectionMode::Surface));
    }

    #[test]
    fn intended_snapshot_only_journals_live_windows_for_os_side_effect_modes() {
        let windows = vec![window(1, "app", "A", Rect::new(0, 0, 100, 100))];
        let records = intended_snapshot(&windows, ProtectionMode::StrictHide);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].window, WindowId(1));
        assert!(intended_snapshot(&windows, ProtectionMode::Surface).is_empty());
    }

    #[test]
    fn snapshot_records_pid_and_skips_surface_mode() {
        let windows = vec![
            window(1, "chrome", "A", Rect::new(0, 0, 100, 100)),
            window(2, "vault", "B", Rect::new(0, 0, 100, 100)),
        ];
        let applied = vec![
            AppliedProtection::applied(WindowId(9), ProtectionMode::StrictHide),
            AppliedProtection::applied(WindowId(2), ProtectionMode::Surface),
            AppliedProtection::failed(WindowId(1), ProtectionMode::Exclusion, "partial apply"),
        ];
        let records = snapshot(&applied, &windows);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].window, WindowId(1));
        assert_eq!(records[0].pid, windows[0].pid);
        assert_eq!(records[0].mode, ProtectionMode::Exclusion);
    }

    #[test]
    fn stale_window_handles_do_not_match() {
        let record = RecoveryRecord {
            window: WindowId(1),
            pid: 100,
            mode: ProtectionMode::StrictHide,
            saved_at_unix: 1,
        };
        let mut windows = vec![window(1, "chrome", "A", Rect::new(0, 0, 100, 100))];
        windows[0].pid = 999;
        assert!(!matches_live(&record, &windows));
        windows[0].pid = 100;
        assert!(matches_live(&record, &windows));
    }

    #[test]
    fn journal_round_trip_and_clear() {
        let dir = std::env::temp_dir().join(format!(
            "veil-recovery-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let journal = RecoveryJournal::new(&dir);
        assert!(journal.load().unwrap().is_empty());

        let records = vec![RecoveryRecord {
            window: WindowId(0x2a),
            pid: 4242,
            mode: ProtectionMode::Exclusion,
            saved_at_unix: 1234,
        }];
        journal.save(&records).unwrap();
        assert_eq!(journal.load().unwrap(), records);

        journal.save(&[]).unwrap();
        assert!(journal.load().unwrap().is_empty());
        assert!(!journal.path().exists());
    }
}
