// Single place where the UI talks to the Rust side.
//
// When the page is opened in a plain browser (`pnpm dev` without Tauri) every
// command falls back to an in-memory mock so the interface can be reviewed and
// screenshotted. The mock never claims to protect anything: the banner says
// "preview data".

import { invoke } from "@tauri-apps/api/core";
import type {
  AppKey,
  AppliedProtection,
  Capabilities,
  Inventory,
  Platform,
  Profile,
  ProtectionMode,
  Rect,
  Rule,
  SurfacePreview,
  TickReport,
  VerificationRun,
  WindowInfo,
} from "./types";

export const IN_TAURI =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

// ---------------------------------------------------------------------------
// mock state
// ---------------------------------------------------------------------------

const DEMO_WINDOWS: { id: number; app: string; appName: string; title: string; pid: number; bounds: Rect }[] = [
  { id: 0x1a, app: "chrome", appName: "chrome", title: "Inbox — work", pid: 4100, bounds: { x: 40, y: 40, w: 820, h: 520 } },
  { id: 0x2b, app: "1password", appName: "1password", title: "Vault", pid: 4112, bounds: { x: 420, y: 240, w: 560, h: 420 } },
  { id: 0x3c, app: "slack", appName: "slack", title: "#general", pid: 4130, bounds: { x: 880, y: 60, w: 360, h: 300 } },
  { id: 0x4d, app: "obs", appName: "obs", title: "OBS 31.0", pid: 4144, bounds: { x: 880, y: 400, w: 360, h: 260 } },
];

const DEMO_COLORS = ["#465a82", "#78465a", "#3c6e5a", "#6e643c"];

let mockProfile: Profile = { version: 1, rules: [], surface: {}, auto_arm: "off" };
let mockNextRule = 1;

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

function mockApplied(): AppliedProtection[] {
  const applied: AppliedProtection[] = [];
  for (const window of mockWindows()) {
    const rule = mockProfile.rules.find(
      (entry) => entry.enabled && appMatches(entry, window),
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

function appMatches(rule: Rule, window: WindowInfo): boolean {
  if (rule.target.kind !== "app") return false;
  return normalizeApp(rule.target.value) === normalizeApp(window.app);
}

function mockInventory(): Inventory {
  return {
    windows: mockWindows(),
    backend: {
      platform: mockPlatform(),
      name: "Preview data",
      live: false,
      note: "Browser preview: no OS APIs are called. Run `pnpm tauri dev` for the live backend.",
    },
    applied: mockApplied(),
  };
}

function mockTick(): TickReport {
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

function mockPreview(): SurfacePreview {
  const windows = mockWindows();
  const protectedApps = new Set(
    mockProfile.rules
      .filter((rule) => rule.enabled && rule.mode === "surface")
      .map((rule) => (rule.target.kind === "app" ? normalizeApp(rule.target.value) : "")),
  );
  const width = 1280;
  const height = 800;
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext("2d");
  if (!ctx) {
    return { png_base64: "", width, height, masked: 0, source: "preview" };
  }
  ctx.fillStyle = "#262a36";
  ctx.fillRect(0, 0, width, height);
  let masked = 0;
  for (const [index, window] of windows.entries()) {
    const { x, y, w, h } = window.bounds;
    ctx.fillStyle = DEMO_COLORS[index % DEMO_COLORS.length];
    ctx.fillRect(x, y, w, h);
    ctx.fillStyle = "#181a21";
    ctx.fillRect(x, y, w, Math.min(28, h));
    if (protectedApps.has(normalizeApp(window.app))) {
      masked += 1;
      for (let ty = y; ty < y + h; ty += 16) {
        for (let tx = x; tx < x + w; tx += 16) {
          const even = ((tx - x) / 16 + (ty - y) / 16) % 2 === 0;
          ctx.fillStyle = even ? "#181a21" : "#262a36";
          ctx.fillRect(tx, ty, Math.min(16, x + w - tx), Math.min(16, y + h - ty));
        }
      }
    }
  }
  const data = canvas.toDataURL("image/png");
  return {
    png_base64: data.slice(data.indexOf(",") + 1),
    width,
    height,
    masked,
    source: "simulated desktop (browser preview)",
  };
}

function mockVerify(): VerificationRun {
  const now = Math.floor(Date.now() / 1000);
  return {
    path: "synthetic",
    baseline: { found: true, confidence: 1, origin: { x: 448, y: 238 }, matched: 64 },
    after: { found: false, confidence: 0, origin: null, matched: 0 },
    outcome: "protected",
    record: {
      path: "synthetic",
      outcome: "protected",
      checked_at_unix: now,
      os_build: "browser-preview",
      detail:
        "Preview data only. The real harness composites an 8x8 canary, captures before arming, re-captures after, and reports Protected only when the baseline saw the canary and the second capture did not.",
    },
  };
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
                  detail: "Needs a desktop-specific geometry provider; falls back to mirroring one window.",
                }
              : { level: "native" as const },
          strict_hide: { level: "native" as const },
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

export async function getProfile(): Promise<Profile> {
  if (!IN_TAURI) return structuredClone(mockProfile);
  return invoke<Profile>("get_profile");
}

export async function addAppRule(
  app: AppKey,
  mode: ProtectionMode,
): Promise<Profile> {
  if (!IN_TAURI) {
    mockProfile.rules = mockProfile.rules.filter(
      (rule) => !(rule.target.kind === "app" && normalizeApp(rule.target.value) === normalizeApp(app)),
    );
    mockProfile.rules.push({ id: mockNextRule++, target: { kind: "app", value: app }, mode, enabled: true, note: "" });
    return structuredClone(mockProfile);
  }
  return invoke<Profile>("add_app_rule", { app, mode });
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
    mockProfile = { version: 1, rules: [], surface: {}, auto_arm: "off" };
    return structuredClone(mockProfile);
  }
  return invoke<Profile>("reset_profile");
}

export async function tick(): Promise<TickReport> {
  if (!IN_TAURI) return mockTick();
  return invoke<TickReport>("tick");
}

export async function releaseAll(): Promise<{ released: number; failed: string[] }> {
  if (!IN_TAURI) {
    mockProfile.rules = [];
    return { released: mockApplied().length, failed: [] };
  }
  return invoke<{ released: number; failed: string[] }>("release_all");
}

export async function preview(): Promise<SurfacePreview> {
  if (!IN_TAURI) return mockPreview();
  return invoke<SurfacePreview>("preview");
}

export async function verify(): Promise<VerificationRun> {
  if (!IN_TAURI) return mockVerify();
  return invoke<VerificationRun>("verify");
}
