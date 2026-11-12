// Mirrors the serde representation of the Rust domain types. Keep in sync with
// `crates/veil-core`; the CLI's `--json` output is the reference.

export type ProtectionMode = "exclusion" | "surface" | "strict_hide";

export type Platform = "windows" | "mac_os" | "linux_x11" | "linux_wayland";

export type CapabilityLevel =
  | { level: "native" }
  | { level: "partial"; detail: string }
  | { level: "unsupported"; reason: string };

export interface Capabilities {
  platform: Platform;
  modes: Record<ProtectionMode, CapabilityLevel>;
}

export type AppKey =
  | { kind: "windows_executable"; value: string }
  | { kind: "mac_bundle"; value: string }
  | { kind: "linux_app_id"; value: string };

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface WindowInfo {
  id: number;
  app: AppKey;
  app_name: string;
  title: string;
  pid: number;
  bounds: Rect;
  monitor: number | null;
  visible: boolean;
  minimized: boolean;
  top_level: boolean;
  platform: Platform;
  /** Base64 PNG of the owning app's icon; empty when unavailable. */
  icon?: string;
}

export type TargetRef =
  | { kind: "app"; value: AppKey }
  | { kind: "window"; value: number };

export interface Rule {
  id: number;
  target: TargetRef;
  mode: ProtectionMode;
  enabled: boolean;
  note: string;
}

export interface Profile {
  version: number;
  rules: Rule[];
  surface: unknown;
  auto_arm: "off" | "capture_app_running" | "always";
  next_rule_id?: number;
}

export interface ApplyStatus {
  status: "applied" | "pending" | "failed";
  reason?: string;
}

export interface AppliedProtection {
  window: number;
  mode: ProtectionMode;
  status: ApplyStatus;
}

export interface BackendInfo {
  platform: Platform;
  name: string;
  live: boolean;
  note: string;
}

export interface Inventory {
  windows: WindowInfo[];
  backend: BackendInfo;
  applied: AppliedProtection[];
}

export interface TickReport {
  applied: AppliedProtection[];
  released: number[];
  failed: [number, string][];
  unchanged: number;
  conflicts: unknown[];
  upgraded_to_surface: number[];
  pruned_rules: number;
}

export interface SurfacePreview {
  png_base64: string;
  width: number;
  height: number;
  masked: number;
  source: string;
}

export interface Detection {
  found: boolean;
  confidence: number;
  origin: { x: number; y: number } | null;
  matched: number;
}

export interface VerificationRecord {
  path: string;
  outcome: "protected" | "visible" | "inconclusive";
  checked_at_unix: number;
  os_build: string;
  detail: string;
}

export interface VerificationRun {
  path: string;
  baseline: Detection;
  after: Detection;
  outcome: "protected" | "visible" | "inconclusive";
  record: VerificationRecord;
}

export const MODES: { id: ProtectionMode; label: string; blurb: string }[] = [
  {
    id: "exclusion",
    label: "Exclusion",
    blurb: "OS removes the window from capture. Windows only.",
  },
  {
    id: "surface",
    label: "Veil Surface",
    blurb: "Share the filtered surface window. Works everywhere.",
  },
  {
    id: "strict_hide",
    label: "Strict hide",
    blurb: "Hide the window locally for the session, restore after.",
  },
];

export const PLATFORM_LABELS: Record<Platform, string> = {
  windows: "Windows",
  mac_os: "macOS",
  linux_x11: "Linux (X11)",
  linux_wayland: "Linux (Wayland)",
};

export function modeLabel(mode: ProtectionMode): string {
  return MODES.find((entry) => entry.id === mode)?.label ?? mode;
}

export function levelBadge(level: CapabilityLevel): string {
  if (level.level === "native") return "native";
  if (level.level === "partial") return "partial";
  return "unsupported";
}

export const LEVEL_BADGES: Record<CapabilityLevel["level"], string> = {
  native: "✓",
  partial: "◐",
  unsupported: "—",
};

export function levelDetail(level: CapabilityLevel): string {
  if (level.level === "native") return "Works for arbitrary third-party windows.";
  if (level.level === "partial") return level.detail;
  return level.reason;
}
