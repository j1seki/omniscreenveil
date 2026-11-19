//! `veil` — command-line agent for OmniScreenVeil.
//!
//! The CLI is the reference implementation of the shared core: it can print the
//! honest capability matrix, run the headless verification self-test, inspect
//! live windows and drive the X11 strict-hide backend. The desktop UI consumes
//! exactly the same crates.

use std::io::Write;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};
use serde::Serialize;
use serde_json::json;

use veil_core::capability::ProtectionMode;
use veil_core::ids::{AppKey, WindowId};
use veil_core::testutil::demo_windows;
use veil_core::{Platform, PlatformCapabilities, Profile, Rect, Size, VerifyOutcome, WindowInfo};
#[cfg(target_os = "linux")]
use veil_protect::TickReport;
use veil_protect::{Engine, MemoryBackend};
use veil_verify::{SyntheticCapture, SyntheticDesktop, VerificationHarness};

#[derive(Parser)]
#[command(
    name = "veil",
    version,
    about = "Selective screen-capture privacy for Windows, macOS and Linux"
)]
struct Cli {
    /// Machine-readable output.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the honest per-platform capability matrix.
    Capabilities {
        #[arg(long, default_value = "auto")]
        platform: String,
    },
    /// Run the headless end-to-end self-test (no display needed).
    Selftest,
    /// List live top-level windows.
    List,
    /// Render a Veil Surface snapshot to a PNG or PPM image.
    Surface {
        #[arg(long, default_value = "veil-surface.ppm")]
        out: PathBuf,
        /// Window ids to mask (hex or decimal).
        #[arg(long, value_delimiter = ',')]
        protect: Vec<String>,
        /// Capture the live desktop.
        #[arg(long)]
        live: bool,
    },
    /// Strict-hide a window on X11, sway, or Hyprland.
    Protect {
        window: String,
        #[arg(long, default_value = "strict-hide")]
        mode: String,
    },
    /// Restore a window hidden with `protect`.
    Release { window: String },
}

fn main() {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("veil: {err:#}");
            std::process::exit(1);
        }
    }
}

fn run(cli: Cli) -> Result<i32> {
    let json = cli.json;
    match cli.command {
        Command::Capabilities { platform } => cmd_capabilities(json, &platform),
        Command::Selftest => cmd_selftest(json),
        Command::List => cmd_list(json),
        Command::Surface { out, protect, live } => cmd_surface(json, &out, &protect, live),
        Command::Protect { window, mode } => cmd_protect(json, &window, &mode),
        Command::Release { window } => cmd_release(json, &window),
    }
}

// ---------------------------------------------------------------------------
// capabilities
// ---------------------------------------------------------------------------

fn parse_platform(value: &str) -> Result<Platform> {
    Ok(match value {
        "auto" | "detect" => Platform::detect(),
        "windows" | "win" => Platform::Windows,
        "macos" | "mac" => Platform::MacOs,
        "x11" | "linux-x11" => Platform::LinuxX11,
        "wayland" | "linux-wayland" => Platform::LinuxWayland,
        other => bail!("unknown platform {other:?} (windows, macos, x11, wayland, auto)"),
    })
}

fn capabilities_for(platform: Platform) -> PlatformCapabilities {
    PlatformCapabilities::for_platform(platform)
}

fn cmd_capabilities(json: bool, platform: &str) -> Result<i32> {
    let platform = parse_platform(platform)?;
    let caps = capabilities_for(platform);
    if json {
        println!("{}", serde_json::to_string_pretty(&caps)?);
        return Ok(0);
    }
    println!("OmniScreenVeil capabilities — {}", platform.label());
    println!();
    println!("{:<14} {:<12} DETAIL", "MODE", "STATUS");
    for mode in ProtectionMode::ALL {
        let level = caps.level(mode);
        println!(
            "{:<14} {:<12} {}",
            mode.label(),
            level.badge(),
            level
                .detail()
                .unwrap_or("works for arbitrary third-party windows")
        );
    }
    println!();
    for mode in ProtectionMode::ALL {
        println!("{}: {}", mode.label(), mode.description());
    }
    println!();
    println!(
        "Best available mode: {}",
        caps.best_mode().map(|m| m.label()).unwrap_or("none")
    );
    Ok(0)
}

// ---------------------------------------------------------------------------
// selftest
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct Check {
    name: String,
    passed: bool,
    detail: String,
}

impl Check {
    fn new(name: impl Into<String>, passed: bool, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed,
            detail: detail.into(),
        }
    }
}

fn cmd_selftest(json: bool) -> Result<i32> {
    let checks = selftest_checks()?;
    let failed = checks.iter().filter(|c| !c.passed).count();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "version": env!("CARGO_PKG_VERSION"),
                "platform": Platform::detect().label(),
                "checks": checks,
                "failed": failed,
                "passed": failed == 0,
            }))?
        );
    } else {
        println!("OmniScreenVeil self-test");
        println!("host: {}", Platform::detect().label());
        println!();
        for check in &checks {
            println!(
                "{} {:<44} {}",
                if check.passed { "PASS" } else { "FAIL" },
                check.name,
                check.detail
            );
        }
        println!();
        if failed == 0 {
            println!("{} checks passed.", checks.len());
        } else {
            println!("{failed} of {} checks FAILED.", checks.len());
        }
    }
    Ok(if failed == 0 { 0 } else { 1 })
}

fn selftest_checks() -> Result<Vec<Check>> {
    let mut checks = Vec::new();

    // 1. Capability matrix invariants -------------------------------------
    let windows = capabilities_for(Platform::Windows);
    let macos = capabilities_for(Platform::MacOs);
    let wayland = capabilities_for(Platform::LinuxWayland);
    checks.push(Check::new(
        "capability matrix stays honest",
        windows.supports(ProtectionMode::Exclusion)
            && !macos.supports(ProtectionMode::Exclusion)
            && !wayland.supports(ProtectionMode::Exclusion),
        "Windows advertises exclusion; macOS and Wayland do not",
    ));
    checks.push(Check::new(
        "unsupported modes downgrade instead of failing",
        macos.effective_mode(ProtectionMode::Exclusion) == ProtectionMode::Surface,
        "macOS Exclusion -> Surface",
    ));

    // 2. Reconcile loop ----------------------------------------------------
    let mut profile = Profile::new();
    profile.add_app_rule(
        AppKey::LinuxAppId("chrome".into()),
        ProtectionMode::Exclusion,
    );
    let live = demo_windows();
    let mut engine = Engine::new(profile.clone(), MemoryBackend::new(Platform::Windows));
    let first = engine.tick(&live);
    checks.push(Check::new(
        "first tick protects every matching window",
        first.applied.len() == 1 && first.failed.is_empty(),
        format!(
            "applied={} released={} failed={}",
            first.applied.len(),
            first.released.len(),
            first.failed.len()
        ),
    ));
    let second = engine.tick(&live);
    checks.push(Check::new(
        "reconcile is idempotent",
        !second.did_work(),
        format!("unchanged={}", second.unchanged),
    ));
    let gone: Vec<_> = live
        .iter()
        .filter(|w| w.app_name != "chrome")
        .cloned()
        .collect();
    let third = engine.tick(&gone);
    checks.push(Check::new(
        "closed windows are released",
        third.released == vec![WindowId(0x1a)],
        format!("released={:?}", third.released),
    ));

    // 3. Capture verification ---------------------------------------------
    let desktop = SyntheticDesktop::new(Size::new(960, 540));
    let token = desktop.token();
    let region = desktop.canary_rect();
    let protected_plan = desktop.protected_plan();
    let mut harness =
        VerificationHarness::new(SyntheticCapture::new(&desktop, None), token, desktop.cell())
            .with_region(region);
    let protected = harness.run_with(|source| {
        source.set_plan(Some(protected_plan.clone()));
        Ok(())
    })?;
    checks.push(Check::new(
        "verified protection: canary present before, absent after",
        protected.outcome == VerifyOutcome::Protected && protected.record.detail.contains("absent"),
        protected.record.detail.clone(),
    ));
    checks.push(Check::new(
        "verification records provenance, not a boolean",
        protected.record.path == veil_core::CapturePath::Synthetic
            && !protected.record.os_build.is_empty()
            && protected.record.checked_at_unix > 0,
        format!(
            "path={} build={}",
            protected.record.path.label(),
            protected.record.os_build
        ),
    ));

    // 4. Fail-closed --------------------------------------------------------
    let leaky_plan = desktop.unprotecting_plan();
    let mut leaky =
        VerificationHarness::new(SyntheticCapture::new(&desktop, None), token, desktop.cell())
            .with_region(region);
    let leaked = leaky.run_with(|source| {
        source.set_plan(Some(leaky_plan.clone()));
        Ok(())
    })?;
    checks.push(Check::new(
        "a missing mask is reported as visible",
        leaked.outcome == VerifyOutcome::Visible,
        leaked.record.detail.clone(),
    ));

    // 5. Release restores content ------------------------------------------
    harness.source.set_plan(None);
    let released = harness.run()?;
    checks.push(Check::new(
        "release restores the canary",
        released.baseline.found && released.after.found,
        "canary visible again after release",
    ));

    // 6. Surface edit correctness ------------------------------------------
    let frame = veil_surface::render(desktop.frame(), &desktop.protected_plan());
    let outside = frame
        .get_pixel(5, 5)
        .map(|c| c == veil_core::Color::rgb(38, 42, 52))
        .unwrap_or(false);
    let inside = frame
        .get_pixel(region.x as u32 + 3, region.y as u32 + 3)
        .map(|c| c != veil_core::Color::rgb(38, 42, 52))
        .unwrap_or(false);
    checks.push(Check::new(
        "surface masks only the protected region",
        outside && inside && frame.width == 960 && frame.height == 540,
        "desktop pixels outside the exclusion are untouched",
    ));

    // 7. Canary determinism -------------------------------------------------
    checks.push(Check::new(
        "canary token is deterministic",
        veil_canary::Token::from_seed("x") == veil_canary::Token::from_seed("x")
            && veil_canary::Token::from_seed("x") != veil_canary::Token::from_seed("y"),
        token.hex(),
    ));

    // 8. Rule pruning -------------------------------------------------------
    let mut profile = profile.clone();
    profile.add_window_rule(WindowId(0x99), ProtectionMode::Surface);
    let pruned = profile.prune_dead_window_rules(&live);
    checks.push(Check::new(
        "dead window rules are pruned",
        pruned == 1 && profile.rules.len() == 1,
        format!("pruned={pruned}"),
    ));

    Ok(checks)
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

fn cmd_list(json: bool) -> Result<i32> {
    let platform = Platform::detect();
    match platform {
        Platform::LinuxX11 => list_x11(json),
        Platform::Windows => {
            let backend = veil_protect_win::WinBackend::new()?;
            let windows = backend.list_windows()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&windows)?);
            } else {
                for w in &windows {
                    println!(
                        "{:<10} {:<8} {:<22} {}",
                        w.id.to_string(),
                        w.pid,
                        truncate(&w.app_name, 22),
                        truncate(&w.display_name(), 60)
                    );
                }
            }
            Ok(0)
        }
        Platform::MacOs => {
            let backend = veil_protect_macos::MacBackend::new()?;
            let windows = backend.list_windows()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&windows)?);
            } else {
                for w in &windows {
                    println!(
                        "{:<10} {:<8} {:<22} {}",
                        w.id.to_string(),
                        w.pid,
                        truncate(&w.app_name, 22),
                        truncate(&w.display_name(), 60)
                    );
                }
            }
            Ok(0)
        }
        Platform::LinuxWayland => list_wayland(json),
    }
}

#[cfg(target_os = "linux")]
fn list_x11(json: bool) -> Result<i32> {
    let platform = veil_protect_linux::X11Platform::connect()
        .context("cannot connect to X11 (is DISPLAY set?)")?;
    let windows = platform.list_windows()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&windows)?);
    } else {
        println!(
            "{:<10} {:<8} {:<22} {:<28} BOUNDS",
            "WINDOW", "PID", "APP", "TITLE"
        );
        for w in &windows {
            println!(
                "{:<10} {:<8} {:<22} {:<28} {:?}",
                w.id.to_string(),
                w.pid,
                truncate(&w.app_name, 22),
                truncate(&w.title, 28),
                w.bounds
            );
        }
    }
    Ok(0)
}

#[cfg(not(target_os = "linux"))]
fn list_x11(_json: bool) -> Result<i32> {
    bail!("X11 inventory is only available on Unix hosts")
}

#[cfg(target_os = "linux")]
fn list_wayland(json: bool) -> Result<i32> {
    let backend = veil_protect_wayland::WaylandBackend::new()?;
    let windows = backend.list_windows()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&windows)?);
    } else {
        println!(
            "{} — {:<10} {:<8} {:<22} {:<28} BOUNDS",
            backend.compositor().label(),
            "WINDOW",
            "PID",
            "APP",
            "TITLE"
        );
        for w in &windows {
            println!(
                "{:<10} {:<8} {:<22} {:<28} {:?}",
                w.id.to_string(),
                w.pid,
                truncate(&w.app_name, 22),
                truncate(&w.title, 28),
                w.bounds
            );
        }
    }
    Ok(0)
}

#[cfg(not(target_os = "linux"))]
fn list_wayland(_json: bool) -> Result<i32> {
    bail!("Wayland inventory is only available on Unix hosts")
}

fn selected_bounds(windows: &[WindowInfo], ids: &[WindowId]) -> Result<Vec<Rect>> {
    ids.iter()
        .map(|id| {
            windows
                .iter()
                .find(|window| window.id == *id)
                .map(|window| window.bounds)
                .ok_or_else(|| anyhow!("window {id} is not a live top-level window"))
        })
        .collect()
}

fn masked_capture(
    frame: veil_surface::Frame,
    masks: impl IntoIterator<Item = Rect>,
) -> (veil_surface::SurfacePlan, veil_surface::Frame) {
    let plan = masks.into_iter().fold(
        veil_surface::SurfacePlan::new(frame.size(), veil_core::FillStyle::veiled()),
        |plan, bounds| plan.exclude(bounds),
    );
    (plan, frame)
}

#[cfg(target_os = "linux")]
fn x11_plan_and_capture(
    ids: &[WindowId],
) -> Result<(veil_surface::SurfacePlan, veil_surface::Frame)> {
    let platform =
        veil_protect_linux::X11Platform::connect().context("--live requires an X11 display")?;
    let bounds = selected_bounds(&platform.list_windows()?, ids)?;
    let size = platform.root_size()?;
    let compositor = platform.has_compositor();
    let own = if compositor {
        Vec::new()
    } else {
        platform.own_windows()?
    };
    let frame = if compositor {
        platform.capture_desktop()?
    } else {
        platform.capture_root()?
    };
    if bounds != selected_bounds(&platform.list_windows()?, ids)?
        || size != platform.root_size()?
        || size != frame.size()
        || compositor != platform.has_compositor()
        || (!compositor && own != platform.own_windows()?)
    {
        bail!("desktop geometry changed during capture; retry the snapshot");
    }
    Ok(masked_capture(frame, bounds.into_iter().chain(own)))
}

#[cfg(not(target_os = "linux"))]
fn x11_plan_and_capture(
    _ids: &[WindowId],
) -> Result<(veil_surface::SurfacePlan, veil_surface::Frame)> {
    bail!("--live requires an X11 display and a Unix host")
}

#[cfg(target_os = "linux")]
fn wayland_plan_and_capture(
    ids: &[WindowId],
) -> Result<(veil_surface::SurfacePlan, veil_surface::Frame)> {
    let backend = veil_protect_wayland::WaylandBackend::new()?;
    let bounds = selected_bounds(&backend.list_windows()?, ids)?;
    let outputs = backend.outputs()?;
    let region = outputs
        .iter()
        .map(|(_, rect)| *rect)
        .reduce(|a, b| a.union(&b))
        .context("could not locate any Wayland outputs")?;
    let own = backend.own_windows()?;
    let frame = backend.capture_region(Some(region))?;
    if bounds != selected_bounds(&backend.list_windows()?, ids)?
        || outputs != backend.outputs()?
        || frame.width != region.w
        || frame.height != region.h
        || own != backend.own_windows()?
    {
        bail!("desktop geometry changed during capture; retry the snapshot");
    }
    Ok(masked_capture(
        frame,
        bounds
            .into_iter()
            .chain(own)
            .map(|bounds| bounds.translated(-region.x, -region.y)),
    ))
}

#[cfg(not(target_os = "linux"))]
fn wayland_plan_and_capture(
    _ids: &[WindowId],
) -> Result<(veil_surface::SurfacePlan, veil_surface::Frame)> {
    bail!("--live Wayland capture is only available on Unix hosts")
}

fn win_plan_and_capture(
    ids: &[WindowId],
) -> Result<(veil_surface::SurfacePlan, veil_surface::Frame)> {
    let backend = veil_protect_win::WinBackend::new()?;
    let bounds = selected_bounds(&backend.inventory(false)?, ids)?;
    let own = backend.own_windows();
    let frame = backend.capture_root()?;
    if bounds != selected_bounds(&backend.inventory(false)?, ids)? || own != backend.own_windows() {
        bail!("desktop geometry changed during capture; retry the snapshot");
    }
    Ok(masked_capture(frame, bounds.into_iter().chain(own)))
}

fn macos_plan_and_capture(
    ids: &[WindowId],
) -> Result<(veil_surface::SurfacePlan, veil_surface::Frame)> {
    let backend = veil_protect_macos::MacBackend::new()?;
    selected_bounds(&backend.inventory(false)?, ids)?;
    let frame = backend.capture_filtered(&ids.iter().copied().collect())?;
    Ok(masked_capture(frame, []))
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let mut out: String = value.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

// ---------------------------------------------------------------------------
// surface
// ---------------------------------------------------------------------------

fn cmd_surface(json: bool, out: &PathBuf, protect: &[String], live: bool) -> Result<i32> {
    let ids: Vec<WindowId> = protect
        .iter()
        .map(|s| parse_window_id(s))
        .collect::<Result<_>>()?;

    let (plan, source) = if live {
        match Platform::detect() {
            Platform::LinuxWayland => wayland_plan_and_capture(&ids)?,
            Platform::Windows => win_plan_and_capture(&ids)?,
            Platform::MacOs => macos_plan_and_capture(&ids)?,
            Platform::LinuxX11 => x11_plan_and_capture(&ids)?,
        }
    } else {
        let size = Size::new(1280, 800);
        let windows = demo_windows();
        let mut plan = veil_surface::SurfacePlan::new(size, veil_core::FillStyle::veiled());
        for id in &ids {
            if let Some(window) = windows.iter().find(|w| w.id == *id) {
                plan = plan.exclude(window.bounds);
            } else {
                bail!("unknown demo window {} (try 0x1a, 0x2b, 0x3c, 0x4d)", id);
            }
        }
        // draw the demo windows so the preview is legible
        let mut source = veil_surface::Frame::filled(
            size.w,
            size.h,
            veil_surface::PixelFormat::Bgra8,
            veil_core::Color::rgb(38, 42, 52),
        );
        for (i, window) in demo_windows().iter().enumerate() {
            let colors = [
                veil_core::Color::rgb(70, 90, 130),
                veil_core::Color::rgb(120, 70, 90),
                veil_core::Color::rgb(60, 110, 90),
                veil_core::Color::rgb(110, 100, 60),
            ];
            source.fill_rect(
                window.bounds,
                veil_core::FillStyle::Solid {
                    color: colors[i % colors.len()],
                },
            );
        }
        (plan, source)
    };

    let rendered = veil_surface::render(&source, &plan);
    write_image(out, &rendered)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "out": out,
                "size": {"w": rendered.width, "h": rendered.height},
                "exclusions": plan.exclusions.len(),
            }))?
        );
    } else {
        println!(
            "wrote {} ({}x{}, {} masked regions)",
            out.display(),
            rendered.width,
            rendered.height,
            plan.exclusions.len()
        );
    }
    Ok(0)
}

fn write_image(path: &PathBuf, frame: &veil_surface::Frame) -> Result<()> {
    let is_png = path
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("png"))
        .unwrap_or(false);
    if is_png {
        return write_png(path, frame);
    }
    write_ppm(path, frame)
}

fn write_png(path: &PathBuf, frame: &veil_surface::Frame) -> Result<()> {
    let file =
        std::fs::File::create(path).with_context(|| format!("cannot create {}", path.display()))?;
    let writer = std::io::BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, frame.width, frame.height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    let mut rgb = Vec::with_capacity(frame.width as usize * frame.height as usize * 3);
    for y in 0..frame.height {
        for x in 0..frame.width {
            let color = frame
                .get_pixel(x, y)
                .ok_or_else(|| anyhow!("pixel {x},{y} out of bounds"))?;
            rgb.extend_from_slice(&[color.r, color.g, color.b]);
        }
    }
    writer.write_image_data(&rgb)?;
    Ok(())
}

fn write_ppm(path: &PathBuf, frame: &veil_surface::Frame) -> Result<()> {
    let mut file =
        std::fs::File::create(path).with_context(|| format!("cannot create {}", path.display()))?;
    write!(file, "P6\n{} {}\n255\n", frame.width, frame.height)?;
    let mut row = Vec::with_capacity(frame.width as usize * 3);
    for y in 0..frame.height {
        row.clear();
        for x in 0..frame.width {
            let color = frame
                .get_pixel(x, y)
                .ok_or_else(|| anyhow!("pixel {x},{y} out of bounds"))?;
            row.extend_from_slice(&[color.r, color.g, color.b]);
        }
        file.write_all(&row)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// protect / release (X11)
// ---------------------------------------------------------------------------

fn parse_window_id(value: &str) -> Result<WindowId> {
    let trimmed = value.trim();
    let parsed = if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16)
    } else {
        trimmed.parse::<u64>()
    };
    parsed
        .map(WindowId)
        .map_err(|_| anyhow!("invalid window id {value:?}"))
}

fn parse_mode(value: &str) -> Result<ProtectionMode> {
    Ok(match value {
        "exclusion" | "exclude" => ProtectionMode::Exclusion,
        "surface" | "veil" => ProtectionMode::Surface,
        "strict-hide" | "hide" | "strict" => ProtectionMode::StrictHide,
        other => bail!("unknown mode {other:?} (exclusion, surface, strict-hide)"),
    })
}

fn cmd_protect(json: bool, window: &str, mode: &str) -> Result<i32> {
    let platform = Platform::detect();
    let id = parse_window_id(window)?;
    let mode = parse_mode(mode)?;
    if mode != ProtectionMode::StrictHide {
        bail!("protect supports strict-hide. Use surface --live --protect <window> for a masked snapshot, or use the desktop app for ongoing protection");
    }
    match platform {
        Platform::LinuxX11 => x11_protect(json, id, mode),
        Platform::LinuxWayland => wayland_protect(json, id, mode),
        Platform::Windows | Platform::MacOs => bail!(
            "Use the desktop app on {}: this short-lived CLI cannot maintain and release protection safely",
            platform.label()
        ),
    }
}

fn cmd_release(json: bool, window: &str) -> Result<i32> {
    let platform = Platform::detect();
    match platform {
        Platform::LinuxX11 => x11_release(json, parse_window_id(window)?),
        Platform::LinuxWayland => wayland_release(json, parse_window_id(window)?),
        _ => bail!("`veil release` drives the Linux backends"),
    }
}

#[cfg(target_os = "linux")]
fn x11_protect(json: bool, id: WindowId, mode: ProtectionMode) -> Result<i32> {
    let backend = veil_protect_linux::X11Backend::connect()?;
    let windows = backend.list_windows()?;
    if !windows.iter().any(|w| w.id == id) {
        bail!("window {id} is not a live top-level window");
    }
    let mut profile = Profile::new();
    profile.add_window_rule(id, mode);
    let mut engine = Engine::new(profile, backend);
    let report = engine.tick(&windows);
    print_report(json, &report)?;
    Ok(if report.failed.is_empty() { 0 } else { 1 })
}

#[cfg(not(target_os = "linux"))]
fn x11_protect(_json: bool, _id: WindowId, _mode: ProtectionMode) -> Result<i32> {
    bail!("X11 backend is not available on this host")
}

#[cfg(target_os = "linux")]
fn x11_release(json: bool, id: WindowId) -> Result<i32> {
    let platform = veil_protect_linux::X11Platform::connect()?;
    platform.set_mapped(id, true)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"released": id}))?
        );
    } else {
        println!("released {id}");
    }
    Ok(0)
}

#[cfg(not(target_os = "linux"))]
fn x11_release(_json: bool, _id: WindowId) -> Result<i32> {
    bail!("X11 backend is not available on this host")
}

#[cfg(target_os = "linux")]
fn wayland_protect(json: bool, id: WindowId, mode: ProtectionMode) -> Result<i32> {
    let backend = veil_protect_wayland::WaylandBackend::new()?;
    let windows = backend.list_windows()?;
    if !windows.iter().any(|w| w.id == id) {
        bail!("window {id} is not a live Wayland top-level window");
    }
    let mut profile = Profile::new();
    profile.add_window_rule(id, mode);
    let mut engine = Engine::new(profile, backend);
    let report = engine.tick(&windows);
    print_report(json, &report)?;
    Ok(if report.failed.is_empty() { 0 } else { 1 })
}

#[cfg(not(target_os = "linux"))]
fn wayland_protect(_json: bool, _id: WindowId, _mode: ProtectionMode) -> Result<i32> {
    bail!("Wayland backend is not available on this host")
}

#[cfg(target_os = "linux")]
fn wayland_release(json: bool, id: WindowId) -> Result<i32> {
    let mut backend = veil_protect_wayland::WaylandBackend::new()?;
    // A fresh CLI process restores a hidden window to the current workspace.
    backend.list_windows()?;
    veil_protect::ProtectionBackend::release(&mut backend, id)
        .map_err(|err| anyhow!(err.to_string()))?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(
                &json!({"released": id, "compositor": backend.compositor().label()})
            )?
        );
    } else {
        println!("released {id} on {}", backend.compositor().label());
    }
    Ok(0)
}

#[cfg(not(target_os = "linux"))]
fn wayland_release(_json: bool, _id: WindowId) -> Result<i32> {
    bail!("Wayland backend is not available on this host")
}

#[cfg(target_os = "linux")]
fn print_report(json: bool, report: &TickReport) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
    println!(
        "applied={} released={} failed={} unchanged={} conflicts={}",
        report.applied.len(),
        report.released.len(),
        report.failed.len(),
        report.unchanged,
        report.conflicts.len()
    );
    for applied in &report.applied {
        println!("  applied {} -> {:?}", applied.window, applied.mode);
    }
    for window in &report.released {
        println!("  released {window}");
    }
    for (window, reason) in &report.failed {
        println!("  failed {window}: {reason}");
    }
    Ok(())
}

// Keep the unused-import checker honest: these types are part of the public
// surface used by the desktop UI and the JSON schema.
#[cfg(test)]
fn _schema_keepalive() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_decimal_and_hex_window_ids() {
        assert_eq!(parse_window_id("26").unwrap(), WindowId(26));
        assert_eq!(parse_window_id("0x1a").unwrap(), WindowId(26));
        assert_eq!(parse_window_id("0X1A").unwrap(), WindowId(26));
        assert!(parse_window_id("nope").is_err());
    }

    #[test]
    fn parses_modes() {
        assert_eq!(parse_mode("exclusion").unwrap(), ProtectionMode::Exclusion);
        assert_eq!(
            parse_mode("strict-hide").unwrap(),
            ProtectionMode::StrictHide
        );
        assert!(parse_mode("magic").is_err());
    }

    #[test]
    fn truncation_is_char_safe() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello", 4), "hel…");
        assert_eq!(truncate("héllo", 3), "hé…");
    }
}
