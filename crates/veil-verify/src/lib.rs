//! Verification is a capture, not a return value.
//!
//! The flow is always the same:
//!
//! 1. put a canary inside the window that should be protected,
//! 2. capture the way an audience would *before* arming (baseline),
//! 3. arm protection,
//! 4. capture again and search for the canary.
//!
//! If the baseline did not contain the canary the result is `Inconclusive` — we
//! never report "protected" when we could not observe the content in the first
//! place.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use veil_canary::{Detection, Token};
use veil_core::{CapturePath, Rect, Size, VerificationRecord, VerifyOutcome};
use veil_surface::{render, Frame, SurfacePlan};

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("capture path unavailable: {0}")]
    Unavailable(String),
    #[error("capture failed: {0}")]
    Failed(String),
}

pub trait CaptureSource {
    fn path(&self) -> CapturePath;
    fn capture(&mut self) -> Result<Frame, CaptureError>;
    fn label(&self) -> String {
        self.path().label().to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerificationRun {
    pub path: CapturePath,
    pub baseline: Detection,
    pub after: Detection,
    pub outcome: VerifyOutcome,
    pub record: VerificationRecord,
}

#[derive(Debug)]
pub struct VerificationHarness<S: CaptureSource> {
    pub source: S,
    pub token: Token,
    pub cell: u32,
    /// Restrict the search to the area the canary must occupy. `None` scans the
    /// whole frame.
    pub search_region: Option<Rect>,
}

impl<S: CaptureSource> VerificationHarness<S> {
    pub fn new(source: S, token: Token, cell: u32) -> Self {
        Self {
            source,
            token,
            cell,
            search_region: None,
        }
    }

    pub fn with_region(mut self, region: Rect) -> Self {
        self.search_region = Some(region);
        self
    }

    pub fn detect(&self, frame: &Frame) -> Detection {
        match self.search_region {
            Some(region) => veil_canary::detect_in(frame, self.token, self.cell, region),
            None => veil_canary::detect(frame, self.token, self.cell),
        }
    }

    pub fn run(&mut self) -> Result<VerificationRun, CaptureError> {
        self.run_with(|_| Ok(()))
    }

    /// Run baseline -> arm -> verify.
    ///
    /// `arm` receives the capture source so the caller can engage protection
    /// between the two captures, which is the only ordering that proves
    /// anything. On real backends this is where `Engine::tick` runs.
    pub fn run_with<F>(&mut self, arm: F) -> Result<VerificationRun, CaptureError>
    where
        F: FnOnce(&mut S) -> Result<(), CaptureError>,
    {
        let baseline_frame = self.source.capture()?;
        let baseline = self.detect(&baseline_frame);
        if !baseline.found {
            let outcome = VerifyOutcome::Inconclusive;
            let record = VerificationRecord::new(self.source.path(), outcome, os_build())
                .with_detail("canary not visible before arming; cannot verify");
            return Ok(VerificationRun {
                path: self.source.path(),
                baseline,
                after: Detection {
                    found: false,
                    confidence: 0.0,
                    origin: None,
                    matched: 0,
                },
                outcome,
                record,
            });
        }

        arm(&mut self.source)?;
        let after_frame = self.source.capture()?;
        let after = self.detect(&after_frame);
        let outcome = if after.found {
            VerifyOutcome::Visible
        } else {
            VerifyOutcome::Protected
        };
        let detail = match (&baseline.origin, &after.origin) {
            (Some(origin), None) => format!(
                "canary at {},{} was present before arming and absent afterwards",
                origin.x, origin.y
            ),
            (_, Some(origin)) => format!(
                "canary still visible at {},{} after arming",
                origin.x, origin.y
            ),
            _ => "canary state could not be compared".to_string(),
        };
        let record =
            VerificationRecord::new(self.source.path(), outcome, os_build()).with_detail(detail);
        Ok(VerificationRun {
            path: self.source.path(),
            baseline,
            after,
            outcome,
            record,
        })
    }
}

/// Best-effort environment string stored next to every verification result.
pub fn os_build() -> String {
    std::env::var("OMV_OS_BUILD").unwrap_or_else(|_| {
        format!(
            "{}-{}-{}",
            std::env::consts::OS,
            std::env::consts::ARCH,
            std::env::consts::FAMILY
        )
    })
}

/// A believable desktop with one canary window, used for headless self-tests and
/// for the UI preview. No display required.
#[derive(Debug, Clone)]
pub struct SyntheticDesktop {
    frame: Frame,
    token: Token,
    cell: u32,
    canary: Rect,
}

impl SyntheticDesktop {
    pub fn new(size: Size) -> Self {
        Self::with_token(size, Token::from_seed("omniscreenveil/selftest"))
    }

    pub fn with_token(size: Size, token: Token) -> Self {
        let cell = 8;
        let mut frame = Frame::filled(
            size.w,
            size.h,
            veil_surface::PixelFormat::Bgra8,
            veil_core::Color::rgb(38, 42, 52),
        );
        // fake desktop content: two "windows" that must stay untouched
        frame.fill_rect(
            Rect::new(20, 20, 180, 120),
            veil_core::FillStyle::Solid {
                color: veil_core::Color::rgb(70, 90, 130),
            },
        );
        frame.fill_rect(
            Rect::new(240, 40, 160, 200),
            veil_core::FillStyle::Solid {
                color: veil_core::Color::rgb(120, 70, 90),
            },
        );
        let pattern = veil_canary::pattern_size(cell);
        let canary = Rect::new(
            ((size.w.saturating_sub(pattern.w)) / 2) as i32,
            ((size.h.saturating_sub(pattern.h)) / 2) as i32,
            pattern.w,
            pattern.h,
        );
        veil_canary::render_at(
            &mut frame,
            token,
            cell,
            veil_core::Point {
                x: canary.x,
                y: canary.y,
            },
        );
        Self {
            frame,
            token,
            cell,
            canary,
        }
    }

    pub fn frame(&self) -> &Frame {
        &self.frame
    }

    pub fn token(&self) -> Token {
        self.token
    }

    pub fn cell(&self) -> u32 {
        self.cell
    }

    pub fn canary_rect(&self) -> Rect {
        self.canary
    }

    /// Plan that hides the canary window and an unrelated region (a second
    /// protected window), leaving the rest of the desktop visible.
    pub fn protected_plan(&self) -> SurfacePlan {
        SurfacePlan::new(self.frame.size(), veil_core::FillStyle::veiled())
            .exclude(self.canary)
            .exclude(Rect::new(20, 20, 180, 120))
    }

    pub fn unprotecting_plan(&self) -> SurfacePlan {
        SurfacePlan::new(self.frame.size(), veil_core::FillStyle::veiled())
            .exclude(Rect::new(20, 20, 180, 120))
    }
}

/// Capture source that renders the synthetic desktop through the real
/// compositor when a plan is armed, exactly as the Veil Surface would.
#[derive(Debug, Clone)]
pub struct SyntheticCapture {
    desktop: Frame,
    plan: Option<SurfacePlan>,
    path: CapturePath,
    captures: usize,
}

impl SyntheticCapture {
    pub fn new(desktop: &SyntheticDesktop, plan: Option<SurfacePlan>) -> Self {
        Self {
            desktop: desktop.frame().clone(),
            plan,
            path: CapturePath::Synthetic,
            captures: 0,
        }
    }

    pub fn set_plan(&mut self, plan: Option<SurfacePlan>) {
        self.plan = plan;
    }

    pub fn captures(&self) -> usize {
        self.captures
    }
}

impl CaptureSource for SyntheticCapture {
    fn path(&self) -> CapturePath {
        self.path
    }

    fn capture(&mut self) -> Result<Frame, CaptureError> {
        self.captures += 1;
        Ok(match &self.plan {
            Some(plan) => render(&self.desktop, plan),
            None => self.desktop.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desktop() -> SyntheticDesktop {
        SyntheticDesktop::new(Size::new(640, 480))
    }

    #[test]
    fn end_to_end_verify_protects_then_reports_failure_when_mask_is_wrong() {
        let d = desktop();
        let plan = d.protected_plan();
        let mut harness =
            VerificationHarness::new(SyntheticCapture::new(&d, None), d.token(), d.cell())
                .with_region(d.canary_rect());
        let run = harness
            .run_with(|source| {
                source.set_plan(Some(plan.clone()));
                Ok(())
            })
            .unwrap();
        assert_eq!(run.outcome, VerifyOutcome::Protected);
        assert!(run.record.detail.contains("absent afterwards"));
        assert!(run.baseline.found);
        assert_eq!(harness.source.captures(), 2);
    }

    #[test]
    fn missing_mask_is_detected_as_visible() {
        let d = desktop();
        let plan = d.unprotecting_plan();
        let mut harness =
            VerificationHarness::new(SyntheticCapture::new(&d, None), d.token(), d.cell())
                .with_region(d.canary_rect());
        let run = harness
            .run_with(|source| {
                source.set_plan(Some(plan.clone()));
                Ok(())
            })
            .unwrap();
        assert_eq!(run.outcome, VerifyOutcome::Visible);
        assert!(run.after.found);
    }

    #[test]
    fn no_baseline_means_inconclusive() {
        let mut frame = Frame::filled(
            320,
            240,
            veil_surface::PixelFormat::Bgra8,
            veil_core::Color::BLACK,
        );
        frame.fill_rect(
            Rect::new(0, 0, 10, 10),
            veil_core::FillStyle::Solid {
                color: veil_core::Color::WHITE,
            },
        );
        let capture = SyntheticCapture {
            desktop: frame,
            plan: None,
            path: CapturePath::Synthetic,
            captures: 0,
        };
        let mut harness = VerificationHarness::new(capture, Token::from_seed("x"), 8);
        let run = harness.run().unwrap();
        assert_eq!(run.outcome, VerifyOutcome::Inconclusive);
        assert_eq!(harness.source.captures(), 1);
    }

    #[test]
    fn release_restores_the_canary() {
        let d = desktop();
        let plan = d.protected_plan();
        let mut harness =
            VerificationHarness::new(SyntheticCapture::new(&d, None), d.token(), d.cell())
                .with_region(d.canary_rect());
        let run = harness
            .run_with(|source| {
                source.set_plan(Some(plan.clone()));
                Ok(())
            })
            .unwrap();
        assert_eq!(run.outcome, VerifyOutcome::Protected);
        harness.source.set_plan(None);
        let run = harness.run().unwrap();
        assert_eq!(run.outcome, VerifyOutcome::Visible);
    }

    #[test]
    fn os_build_is_non_empty() {
        assert!(!os_build().is_empty());
    }
}
