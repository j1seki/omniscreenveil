// Single place where the UI talks to the Rust side.
//
// When the page is opened in a plain browser (`pnpm dev` without Tauri) every
// command falls back to an in-memory mock so the interface can be reviewed and
// screenshotted. The mock never claims to protect anything: the banner says
// "preview data".

import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import pkg from "../package.json";
import type {
  AppKey,
  AppliedProtection,
  AutoArm,
  Capabilities,
  Display,
  DesktopPreferences,
  DiagnosticState,
  InstallOutcome,
  Inventory,
  LogEntry,
  MacPermission,
  Platform,
  PreferenceChanges,
  Profile,
  ProtectionMode,
  Rect,
  ReleaseReport,
  Rule,
  CaptureFrame,
  StreamStatus,
  TickReport,
  UpdateChannel,
  UpdateCheck,
  WindowInfo,
} from "./types";
import { isCaptureWindow } from "./types";

export const IN_TAURI =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

// ---------------------------------------------------------------------------
// mock state
// ---------------------------------------------------------------------------

const DEMO_WINDOWS: { id: number; app: string; appName: string; title: string; pid: number; bounds: Rect }[] = [
  { id: 0x1a, app: "chrome", appName: "chrome", title: "Inbox — work", pid: 4100, bounds: { x: 40, y: 40, w: 820, h: 520 } },
  { id: 0x2b, app: "1password", appName: "1password", title: "Vault — Production Keys", pid: 4112, bounds: { x: 420, y: 240, w: 560, h: 420 } },
  { id: 0x3c, app: "slack", appName: "slack", title: "#security-ops", pid: 4130, bounds: { x: 880, y: 60, w: 360, h: 300 } },
  { id: 0x4d, app: "obs", appName: "obs", title: "OBS Studio 31.0 — Live Scene", pid: 4144, bounds: { x: 880, y: 400, w: 360, h: 260 } },
];

const DEMO_COLORS = ["#3b5998", "#7c3f68", "#2d6e5e", "#7a6234"];

let mockProfile: Profile = { version: 1, rules: [], surface: {}, auto_arm: "off" };
let mockNextRule = 1;
let mockExplicitArmed = false;
let mockPaused = false;
export const PREVIEW_PREFERENCES_KEY = "veil-desktop-settings";
const defaultPreferences: DesktopPreferences = {
  close_behavior: "quit",
  preview: { playback: "automatic", display: "", max_edge: 960, interval_ms: 2000 },
  stream: { mode: "snapshot", display: "", max_edge: 1280, fps: 12 },
};
let mockPreferences = defaultPreferences;
let mockLogs: LogEntry[] = [];

function mockPlatform(): Platform {
  const agent = navigator.userAgent.toLowerCase();
  if (agent.includes("mac")) return "mac_os";
  if (agent.includes("win")) return "windows";
  return "linux_x11";
}

function appKey(app: string): AppKey {
  const platform = mockPlatform();
  if (platform === "windows") {
    return { kind: "windows_executable", value: `C:/Apps/${app}.exe` };
  }
  if (platform === "mac_os") {
    return { kind: "mac_bundle", value: `app.${app}` };
  }
  return { kind: "linux_app_id", value: app };
}

function mockWindows(): WindowInfo[] {
  const platform = mockPlatform();
  return DEMO_WINDOWS.map((window) => ({
    id: window.id,
    app: appKey(window.app),
    app_name: window.appName,
    title: window.title,
    pid: window.pid,
    bounds: window.bounds,
    monitor: null,
    visible: true,
    minimized: false,
    top_level: true,
    platform,
  }));
}

function mockCapabilities(): Capabilities {
  const platform = mockPlatform();
  const unsupported = (reason: string) => ({ level: "unsupported" as const, reason });
  if (platform === "windows") {
    return {
      platform,
      modes: {
        exclusion: { level: "native" },
        surface: { level: "native" },
        strict_hide: { level: "native" },
      },
    };
  }
  if (platform === "mac_os") {
    return {
      platform,
      modes: {
        exclusion: unsupported(
          "No public API lets a third-party process exclude another application's window from ScreenCaptureKit.",
        ),
        surface: { level: "native" },
        strict_hide: { level: "native" },
      },
    };
  }
  return {
    platform,
    modes: {
      exclusion: unsupported("Requires a privileged Xorg XACE module; not shippable."),
      surface: { level: "native" },
      strict_hide: { level: "native" },
    },
  };
}

function normalizeApp(app: AppKey): string {
  return app.value.replaceAll("\\", "/").toLowerCase();
}

function ruleMatchesWindow(rule: Rule, window: WindowInfo): boolean {
  if (rule.target.kind === "window") {
    return rule.target.value === window.id;
  }
  return normalizeApp(rule.target.value) === normalizeApp(window.app);
}

function mockShouldArm(windows: WindowInfo[]): boolean {
  if (mockPaused) return false;
  if (mockProfile.auto_arm === "always") return true;
  if (mockProfile.auto_arm === "capture_app_running") {
    return mockExplicitArmed || windows.some(isCaptureWindow);
  }
  return mockExplicitArmed;
}

function mockApplied(): AppliedProtection[] {
  const windows = mockWindows();
  if (!mockShouldArm(windows)) return [];
  const applied: AppliedProtection[] = [];
  for (const window of windows) {
    const rule = mockProfile.rules.find(
      (entry) => entry.enabled && ruleMatchesWindow(entry, window),
    );
    if (rule) {
      applied.push({
        window: window.id,
        mode: rule.mode,
        status: { status: "applied" },
      });
    }
  }
  return applied;
}

function mockInventory(): Inventory {
  const windows = mockWindows();
  const applied = mockApplied();
  return {
    windows,
    backend: {
      platform: mockPlatform(),
      name: "Preview data",
      live: false,
      note: "Browser preview: no OS APIs are called. Run `pnpm tauri dev` for the live backend.",
    },
    applied,
    armed: mockShouldArm(windows),
    capture_active: windows.some(isCaptureWindow),
    automation_paused: mockPaused,
  };
}

function mockTick(arm = true): TickReport {
  if (arm) { mockPaused = false; mockExplicitArmed = true; }
  const applied = mockApplied();
  return {
    applied,
    released: [],
    failed: [],
    unchanged: applied.length,
    conflicts: [],
    upgraded_to_surface: [],
    pruned_rules: 0,
  };
}

async function mockPreview(maxEdge: number): Promise<CaptureFrame> {
  const windows = mockWindows();
  const applied = mockApplied();
  const surfaceIds = new Set(
    applied.filter((a) => a.mode === "surface").map((a) => a.window),
  );
  const excluded = new Set(applied.filter((a) => a.mode === "exclusion").map((a) => a.window));
  const hidden = new Set(applied.filter((a) => a.mode === "strict_hide").map((a) => a.window));
  const scale = Math.min(1, maxEdge / 1280);
  const width = Math.floor(1280 * scale);
  const height = Math.floor(800 * scale);
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("Browser preview is unavailable.");
  ctx.scale(width / 1280, height / 800);
  ctx.fillStyle = "#121622";
  ctx.fillRect(0, 0, 1280, 800);
  let masked = 0;
  for (const [index, window] of windows.entries()) {
    if (excluded.has(window.id) || hidden.has(window.id)) continue;
    const { x, y, w, h } = window.bounds;
    ctx.fillStyle = DEMO_COLORS[index % DEMO_COLORS.length];
    ctx.fillRect(x, y, w, h);
    ctx.fillStyle = "#0b0e17";
    ctx.fillRect(x, y, w, Math.min(28, h));
    if (surfaceIds.has(window.id)) {
      masked += 1;
      for (let ty = y; ty < y + h; ty += 16) {
        for (let tx = x; tx < x + w; tx += 16) {
          const even = ((tx - x) / 16 + (ty - y) / 16) % 2 === 0;
          ctx.fillStyle = even ? "#181a22" : "#222632";
          ctx.fillRect(tx, ty, Math.min(16, x + w - tx), Math.min(16, y + h - ty));
        }
      }
    }
  }
  const image = await new Promise<Blob>((resolve, reject) => {
    canvas.toBlob((blob) => blob ? resolve(blob) : reject(new Error("Could not encode the preview.")), "image/png");
  });
  return { image, width, height, masked, excluded: excluded.size, armed: mockShouldArm(windows) };
}

// ---------------------------------------------------------------------------
// commands
// ---------------------------------------------------------------------------

export async function capabilities(): Promise<Capabilities> {
  if (!IN_TAURI) return mockCapabilities();
  return invoke<Capabilities>("capabilities");
}

export async function capabilityMatrix(): Promise<Capabilities[]> {
  if (!IN_TAURI) {
    return ["windows", "mac_os", "linux_x11", "linux_wayland"].map((platform) => {
      if (platform === "windows") {
        return {
          platform: "windows" as Platform,
          modes: {
            exclusion: { level: "native" as const },
            surface: { level: "native" as const },
            strict_hide: { level: "native" as const },
          },
        };
      }
      if (platform === "mac_os") {
        return {
          platform: "mac_os" as Platform,
          modes: {
            exclusion: {
              level: "unsupported" as const,
              reason: "No public ScreenCaptureKit exclusion API for foreign windows.",
            },
            surface: { level: "native" as const },
            strict_hide: { level: "native" as const },
          },
        };
      }
      return {
        platform: platform as Platform,
        modes: {
          exclusion: {
            level: "unsupported" as const,
            reason: "Requires a privileged Xorg XACE module or compositor support.",
          },
          surface:
            platform === "linux_wayland"
              ? {
                  level: "partial" as const,
                  detail:
                    "Requires grim and sway or Hyprland compositor IPC. Other Wayland compositors show a labelled simulation.",
                }
              : { level: "native" as const },
          strict_hide:
            platform === "linux_wayland"
              ? {
                  level: "partial" as const,
                  detail:
                    "Works on sway and Hyprland through compositor IPC; GNOME Shell and KDE Plasma need a desktop-specific extension.",
                }
              : { level: "native" as const },
        },
      };
    });
  }
  return invoke<Capabilities[]>("capability_matrix");
}

export async function inventory(): Promise<Inventory> {
  if (!IN_TAURI) return mockInventory();
  return invoke<Inventory>("inventory");
}

export function macPermission(permission: MacPermission, settings = false): Promise<void> {
  return invoke("mac_permission", { permission, settings });
}

export async function getProfile(): Promise<Profile> {
  if (!IN_TAURI) return structuredClone(mockProfile);
  return invoke<Profile>("get_profile");
}

export async function setAutoArm(autoArm: AutoArm): Promise<Profile> {
  if (!IN_TAURI) {
    mockProfile.auto_arm = autoArm;
    mockPaused = false;
    return structuredClone(mockProfile);
  }
  return invoke<Profile>("set_auto_arm", { autoArm });
}

export async function addAppRule(
  app: AppKey,
  mode: ProtectionMode,
  enabled = true,
): Promise<Profile> {
  if (!IN_TAURI) {
    mockProfile.rules = mockProfile.rules.filter(
      (rule) => !(rule.target.kind === "app" && normalizeApp(rule.target.value) === normalizeApp(app)),
    );
    mockProfile.rules.push({ id: mockNextRule++, target: { kind: "app", value: app }, mode, enabled, note: "" });
    return structuredClone(mockProfile);
  }
  return invoke<Profile>("add_app_rule", { app, mode, enabled });
}

export async function addWindowRule(
  window: number,
  mode: ProtectionMode,
): Promise<Profile> {
  if (!IN_TAURI) {
    mockProfile.rules = mockProfile.rules.filter(
      (rule) => !(rule.target.kind === "window" && rule.target.value === window),
    );
    mockProfile.rules.push({ id: mockNextRule++, target: { kind: "window", value: window }, mode, enabled: true, note: "" });
    return structuredClone(mockProfile);
  }
  return invoke<Profile>("add_window_rule", { window, mode });
}

export async function protectAll(mode: ProtectionMode): Promise<TickReport> {
  if (!IN_TAURI) {
    mockExplicitArmed = true;
    for (const window of mockWindows()) {
      const existing = mockProfile.rules.find((rule) => ruleMatchesWindow(rule, window));
      if (existing) {
        existing.enabled = true;
      } else {
        mockProfile.rules.push({
          id: mockNextRule++,
          target: { kind: "app", value: window.app },
          mode,
          enabled: true,
          note: "",
        });
      }
    }
    return mockTick();
  }
  return invoke<TickReport>("protect_all", { mode });
}

export async function removeAppRules(app: AppKey): Promise<Profile> {
  if (!IN_TAURI) {
    mockProfile.rules = mockProfile.rules.filter(
      (rule) => !(rule.target.kind === "app" && normalizeApp(rule.target.value) === normalizeApp(app)),
    );
    return structuredClone(mockProfile);
  }
  return invoke<Profile>("remove_app_rules", { app });
}

export async function removeWindowRules(window: number): Promise<Profile> {
  if (!IN_TAURI) {
    mockProfile.rules = mockProfile.rules.filter(
      (rule) => !(rule.target.kind === "window" && rule.target.value === window),
    );
    return structuredClone(mockProfile);
  }
  return invoke<Profile>("remove_window_rules", { window });
}

export async function removeRule(id: number): Promise<Profile> {
  if (!IN_TAURI) {
    mockProfile.rules = mockProfile.rules.filter((rule) => rule.id !== id);
    return structuredClone(mockProfile);
  }
  return invoke<Profile>("remove_rule", { id });
}

export async function setRuleMode(id: number, mode: ProtectionMode): Promise<Profile> {
  if (!IN_TAURI) {
    const rule = mockProfile.rules.find((entry) => entry.id === id);
    if (rule) rule.mode = mode;
    return structuredClone(mockProfile);
  }
  return invoke<Profile>("set_rule_mode", { id, mode });
}

export async function setRuleEnabled(id: number, enabled: boolean): Promise<Profile> {
  if (!IN_TAURI) {
    const rule = mockProfile.rules.find((entry) => entry.id === id);
    if (rule) rule.enabled = enabled;
    return structuredClone(mockProfile);
  }
  return invoke<Profile>("set_rule_enabled", { id, enabled });
}

export async function resetProfile(): Promise<Profile> {
  if (!IN_TAURI) {
    mockExplicitArmed = false;
    mockProfile = { version: 1, rules: [], surface: {}, auto_arm: "off" };
    return structuredClone(mockProfile);
  }
  return invoke<Profile>("reset_profile");
}

export async function tick(arm = true): Promise<TickReport> {
  if (!IN_TAURI) return mockTick(arm);
  return invoke<TickReport>("tick", { arm });
}

export async function releaseAll(): Promise<ReleaseReport> {
  if (!IN_TAURI) {
    const released = mockApplied().length;
    mockExplicitArmed = false;
    mockPaused = true;
    return { released, failed: [], remaining: 0, automation_paused: true };
  }
  return invoke<ReleaseReport>("release_all");
}

export function readCaptureFrame(buffer: ArrayBuffer): CaptureFrame {
  // One armed flag, followed by four little-endian u32 fields and the PNG.
  const headerBytes = 17;
  if (buffer.byteLength <= headerBytes) throw new Error("Incomplete capture frame.");
  const header = new DataView(buffer);
  return {
    image: new Blob([new Uint8Array(buffer, headerBytes)], { type: "image/png" }),
    armed: header.getUint8(0) === 1,
    width: header.getUint32(1, true),
    height: header.getUint32(5, true),
    masked: header.getUint32(9, true),
    excluded: header.getUint32(13, true),
  };
}

export async function preview(monitor: string | null = null, maxEdge = 1920, liveOnly = false): Promise<CaptureFrame> {
  if (!IN_TAURI) {
    if (liveOnly) throw new Error("Open the desktop app to test real capture.");
    return mockPreview(maxEdge);
  }
  return readCaptureFrame(await invoke<ArrayBuffer>("preview", { monitor, maxEdge, liveOnly }));
}

export function openStream(monitor: string, maxEdge: number): Promise<number> {
  return invoke("open_stream", { monitor, maxEdge });
}

export function nextStreamFrame(id: number): Promise<ArrayBuffer> {
  return invoke("next_stream_frame", { id });
}

export function closeStream(id: number): Promise<void> {
  return invoke("close_stream", { id });
}

export function streamEnabled(): Promise<boolean> {
  return IN_TAURI ? invoke("stream_enabled") : Promise.resolve(false);
}

export function setStreamEnabled(enabled: boolean): Promise<StreamStatus> {
  return invoke("set_stream_enabled", { enabled });
}

export function openSurface(): Promise<void> {
  return invoke("open_surface");
}

export function closeSurface(): Promise<void> {
  return invoke("close_surface");
}

export async function surfaceDisplays(): Promise<Display[]> {
  return IN_TAURI ? invoke("surface_displays") : [];
}

export async function nativeWindowEvents(): Promise<boolean> {
  return IN_TAURI && invoke<boolean>("native_window_events");
}

export async function getPreferences(): Promise<DesktopPreferences> {
  if (IN_TAURI) return invoke("get_preferences");
  try {
    const json = localStorage.getItem(PREVIEW_PREFERENCES_KEY);
    if (json) {
      const saved = JSON.parse(json);
      const stream = saved.stream ?? {};
      const native = stream.mode === "native";
      const savedFps = native ? stream.native_fps : stream.snapshot_interval_ms > 0 ? Math.round(1000 / stream.snapshot_interval_ms) : undefined;
      mockPreferences = {
        close_behavior: saved.close_behavior ?? defaultPreferences.close_behavior,
        preview: {
          ...defaultPreferences.preview,
          ...(stream.mode === "manual" ? { playback: "manual" } : {}),
          display: stream.display ?? defaultPreferences.preview.display,
          ...saved.preview,
        },
        stream: {
          mode: native ? "native" : "snapshot",
          display: stream.display ?? defaultPreferences.stream.display,
          max_edge: stream.max_edge ?? (native ? stream.native_edge : stream.snapshot_edge) ?? defaultPreferences.stream.max_edge,
          fps: Math.max(1, Math.min(60, stream.fps ?? savedFps ?? defaultPreferences.stream.fps)),
        },
      };
    }
  } catch { /* The browser preview can also run with storage disabled. */ }
  return mockPreferences;
}

export async function savePreferences(changes: PreferenceChanges): Promise<DesktopPreferences> {
  if (IN_TAURI) return invoke("save_preferences", { changes });
  const current = await getPreferences();
  mockPreferences = {
    ...current,
    ...changes,
    preview: { ...current.preview, ...changes.preview },
    stream: { ...current.stream, ...changes.stream },
  };
  try { localStorage.setItem(PREVIEW_PREFERENCES_KEY, JSON.stringify(mockPreferences)); }
  catch { /* Keep the settings in memory. */ }
  return mockPreferences;
}

export async function loginStartup(enabled?: boolean): Promise<boolean> {
  return IN_TAURI ? invoke("login_startup", { enabled }) : false;
}

export async function getLogs(): Promise<LogEntry[]> {
  return IN_TAURI ? invoke("get_logs") : [...mockLogs];
}

export async function clearLogs(): Promise<void> {
  if (IN_TAURI) return invoke("clear_logs");
  mockLogs = [];
}

export async function copyLogs(text: string): Promise<void> {
  if (IN_TAURI) return invoke("copy_logs", { text });
  await navigator.clipboard.writeText(text);
}

export function recordLog(level: Exclude<LogEntry["level"], "trace">, message: string) {
  if (IN_TAURI) {
    void invoke("record_frontend_log", { level, message }).catch(() => {});
  } else {
    mockLogs.push({ timestamp: Date.now(), level, source: "dashboard", message });
    if (mockLogs.length > 1_000) mockLogs.shift();
  }
}

export async function openDevTools(): Promise<void> {
  return invoke("open_devtools");
}

// ---------------------------------------------------------------------------
// updates
// ---------------------------------------------------------------------------

export async function appVersion(): Promise<string> {
  if (!IN_TAURI) return `${pkg.version} (preview)`;
  return getVersion();
}

export async function checkForUpdate(
  channel: UpdateChannel,
): Promise<UpdateCheck> {
  if (!IN_TAURI) return { update: null, manifest_found: false };
  return invoke<UpdateCheck>("check_update", { channel });
}

export async function installUpdate(
  channel: UpdateChannel,
): Promise<InstallOutcome> {
  if (!IN_TAURI) {
    throw new Error("Updates can only be installed in the desktop app.");
  }
  return invoke<InstallOutcome>("install_update", { channel });
}

export async function restartApp(): Promise<void> {
  if (!IN_TAURI) return;
  return invoke<void>("restart_app");
}

export function setFeedbackAreas(areas: Rect[]): Promise<void> {
  return invoke("set_feedback_areas", { areas });
}

export function diagnosticState(): Promise<DiagnosticState | null> {
  return IN_TAURI ? invoke("diagnostic_state") : Promise.resolve(null);
}

export function runVerification() {
  return invoke<import("./types").VerificationCheck[]>("run_verification");
}
