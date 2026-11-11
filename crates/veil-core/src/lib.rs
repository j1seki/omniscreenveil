//! Platform-neutral domain model for OmniScreenVeil.
//!
//! Everything in this crate is pure data plus deterministic logic, so it can be
//! unit-tested on any host, including headless CI. Platform backends translate
//! [`WindowInfo`] into these types, and the [`reconcile`] module decides what to
//! apply or release on every tick.

#![forbid(unsafe_code)]

pub mod capability;
pub mod config;
pub mod geometry;
pub mod ids;
pub mod reconcile;
pub mod rules;
pub mod state;
pub mod style;
pub mod testutil;
pub mod verification;
pub mod window;

pub use capability::{CapabilityLevel, ModeCapability, PlatformCapabilities, ProtectionMode};
pub use config::ConfigStore;
pub use geometry::{Point, Rect, Scale, Size};
pub use ids::{AppKey, RuleId, WindowId};
pub use reconcile::{plan, Conflict, ReconcilePlan};
pub use rules::{AutoArm, Profile, Rule, SurfaceSettings, TargetRef};
pub use state::{AppliedProtection, ApplyStatus, ProtectionIntent};
pub use style::{Color, FillStyle};
pub use verification::{CapturePath, VerificationRecord, VerifyOutcome};
pub use window::{Platform, WindowInfo};

/// Semver-ish version of the on-disk profile format.
pub const PROFILE_FORMAT_VERSION: u32 = 1;
