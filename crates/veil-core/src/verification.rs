use serde::{Deserialize, Serialize};

/// The concrete path used to capture pixels. Verification is only meaningful
/// per path: an excluded window can disappear from Windows Graphics Capture
/// while still showing up in a legacy GDI grab, or vice versa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapturePath {
    WindowsGraphicsCapture,
    DxgiDesktopDuplication,
    GdiBitBlt,
    ScreenCaptureKit,
    XComposite,
    XShm,
    X11GetImage,
    PipeWirePortal,
    Synthetic,
}

impl CapturePath {
    pub fn label(self) -> &'static str {
        match self {
            CapturePath::WindowsGraphicsCapture => "Windows.Graphics.Capture",
            CapturePath::DxgiDesktopDuplication => "DXGI Desktop Duplication",
            CapturePath::GdiBitBlt => "GDI BitBlt",
            CapturePath::ScreenCaptureKit => "ScreenCaptureKit",
            CapturePath::XComposite => "XComposite",
            CapturePath::XShm => "MIT-SHM",
            CapturePath::X11GetImage => "X11 GetImage",
            CapturePath::PipeWirePortal => "PipeWire portal",
            CapturePath::Synthetic => "Synthetic (self-test)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifyOutcome {
    /// The canary was present before arming and is absent afterwards.
    Protected,
    /// The canary is still observable through this capture path.
    Visible,
    /// We could not establish a baseline (window hidden, capture failed, ...),
    /// so no claim can be made.
    Inconclusive,
}

impl VerifyOutcome {
    pub fn is_pass(self) -> bool {
        matches!(self, VerifyOutcome::Protected)
    }

    pub fn badge(self) -> &'static str {
        match self {
            VerifyOutcome::Protected => "protected",
            VerifyOutcome::Visible => "visible",
            VerifyOutcome::Inconclusive => "inconclusive",
        }
    }
}

/// Evidence attached to a protection claim. Never store a bare boolean:
/// "protected" without a path, a timestamp and an OS build ages badly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationRecord {
    pub path: CapturePath,
    pub outcome: VerifyOutcome,
    pub checked_at_unix: u64,
    pub os_build: String,
    #[serde(default)]
    pub detail: String,
}

impl VerificationRecord {
    pub fn new(path: CapturePath, outcome: VerifyOutcome, os_build: impl Into<String>) -> Self {
        Self {
            path,
            outcome,
            checked_at_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            os_build: os_build.into(),
            detail: String::new(),
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }
}

/// Aggregate a set of per-path results into the claim shown in the UI.
///
/// Fail-closed: if any tested path still shows the canary, the aggregate is
/// `Visible`, regardless of how many other paths passed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationSummary {
    pub records: Vec<VerificationRecord>,
}

impl VerificationSummary {
    pub fn add(&mut self, record: VerificationRecord) {
        self.records.push(record);
    }

    pub fn overall(&self) -> VerifyOutcome {
        if self
            .records
            .iter()
            .any(|r| r.outcome == VerifyOutcome::Visible)
        {
            return VerifyOutcome::Visible;
        }
        if self
            .records
            .iter()
            .any(|r| r.outcome == VerifyOutcome::Protected)
        {
            return VerifyOutcome::Protected;
        }
        VerifyOutcome::Inconclusive
    }

    pub fn latest(&self) -> Option<&VerificationRecord> {
        self.records.iter().max_by_key(|r| r.checked_at_unix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fail_closed_summary() {
        let mut summary = VerificationSummary::default();
        assert_eq!(summary.overall(), VerifyOutcome::Inconclusive);
        summary.add(VerificationRecord::new(
            CapturePath::WindowsGraphicsCapture,
            VerifyOutcome::Protected,
            "22631",
        ));
        assert_eq!(summary.overall(), VerifyOutcome::Protected);
        summary.add(VerificationRecord::new(
            CapturePath::GdiBitBlt,
            VerifyOutcome::Visible,
            "22631",
        ));
        assert_eq!(summary.overall(), VerifyOutcome::Visible);
    }
}
