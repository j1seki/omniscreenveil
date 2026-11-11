use serde::{Deserialize, Serialize};

use crate::capability::ProtectionMode;
use crate::geometry::Rect;
use crate::ids::{AppKey, RuleId, WindowId};

/// What should be true for a window right now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtectionIntent {
    pub window: WindowId,
    pub app: AppKey,
    pub app_name: String,
    pub pid: u32,
    pub bounds: Rect,
    pub mode: ProtectionMode,
    pub rule: RuleId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ApplyStatus {
    Applied,
    Pending,
    Failed { reason: String },
}

impl ApplyStatus {
    pub fn is_applied(&self) -> bool {
        matches!(self, ApplyStatus::Applied)
    }
}

/// What the backend believes is true for a window right now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedProtection {
    pub window: WindowId,
    pub mode: ProtectionMode,
    pub status: ApplyStatus,
}

impl AppliedProtection {
    pub fn applied(window: WindowId, mode: ProtectionMode) -> Self {
        Self {
            window,
            mode,
            status: ApplyStatus::Applied,
        }
    }

    pub fn failed(window: WindowId, mode: ProtectionMode, reason: impl Into<String>) -> Self {
        Self {
            window,
            mode,
            status: ApplyStatus::Failed {
                reason: reason.into(),
            },
        }
    }
}
