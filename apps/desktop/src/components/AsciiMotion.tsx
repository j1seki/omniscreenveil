import { useEffect, useMemo, useState } from "react";
import type { Inventory, Profile, ProtectionMode, VerificationRun, WindowInfo } from "../types";
import { findRuleForWindow, isCaptureWindow, modeLabel } from "../types";
import { cn } from "../lib/utils";

const BRAILLE_WAVE = ["⠀", "⠁", "⠃", "⠇", "⠏", "⠟", "⠿", "⡿", "⣿", "⡿", "⠿", "⠟", "⠏", "⠇", "⠃", "⠁"];
const BLOCK_SHADES = ["·", "∙", "░", "▒", "▓", "█", "▓", "▒", "░", "∙"];
const HEX_GLYPHS = "0123456789ABCDEF";

/**
 * Live animated OpenCode-style ASCII / Braille wave & shield telemetry strip.
 */
export function AsciiWaveBanner({
  armed,
  protectedCount,
  totalWindows,
  captureActive,
}: {
  armed: boolean;
  protectedCount: number;
  totalWindows: number;
  captureActive: boolean;
}) {
  const [tick, setTick] = useState(0);

  useEffect(() => {
    const id = window.setInterval(() => {
      setTick((t) => (t + 1) % 1024);
    }, 90);
    return () => window.clearInterval(id);
  }, []);

  const columns = 22;

  const rowTop = useMemo(() => {
    const chars: { ch: string; tone: string }[] = [];
    for (let i = 0; i < columns; i++) {
      const phase = (i + tick) % BRAILLE_WAVE.length;
      const shadeIdx = (i * 2 + tick) % BLOCK_SHADES.length;
      const ch = armed ? BLOCK_SHADES[shadeIdx] : BRAILLE_WAVE[phase];
      let tone = "text-ink-faint/50";
      if (armed) {
        const mod = (i + tick) % 3;
        tone =
          mod === 0
            ? "text-accent"
            : mod === 1
              ? "text-good"
              : "text-cyan";
      } else if (captureActive) {
        tone = (i + tick) % 2 === 0 ? "text-warn" : "text-bad/80";
      } else if ((i + tick) % 5 === 0) {
        tone = "text-accent/60";
      }
      chars.push({ ch, tone });
    }
    return chars;
  }, [armed, captureActive, tick]);

  const rowBottom = useMemo(() => {
    const chars: string[] = [];
    for (let i = 0; i < columns; i++) {
      if (armed) {
        const idx = (i * 7 + tick * 3) % HEX_GLYPHS.length;
        chars.push(i % 3 === 0 ? "⣿" : HEX_GLYPHS[idx]);
      } else {
        const phase = (columns - i + tick) % BRAILLE_WAVE.length;
        chars.push(BRAILLE_WAVE[phase]);
      }
    }
    return chars.join("");
  }, [armed, tick]);

  const shieldArt = armed
    ? ["╔═╦═╗", "║◈╬◈║", "╚═╩═╝"]
    : captureActive
      ? ["┌─▲─┐", "│ ! │", "└───┘"]
      : ["┌───┐", "│ · │", "└───┘"];

  return (
    <div
      aria-hidden="true"
      className={cn(
        "hidden lg:flex items-center gap-2.5 rounded-lg border px-2.5 py-1 font-mono text-[10px] leading-none select-none transition-colors shrink-0",
        armed
          ? "border-accent/40 bg-accent/[0.06] shadow-[0_0_20px_rgba(0,245,212,0.08)]"
          : captureActive
            ? "border-warn/40 bg-warn/[0.06]"
            : "border-line-soft bg-void/70",
      )}
    >
      <div
        className={cn(
          "flex flex-col text-[9px] leading-[10px] font-bold",
          armed ? "text-accent" : captureActive ? "text-warn" : "text-ink-faint",
        )}
      >
        <span>{shieldArt[0]}</span>
        <span>{shieldArt[1]}</span>
        <span>{shieldArt[2]}</span>
      </div>

      <div className="flex flex-col gap-1">
        <div className="flex items-center tracking-[1.5px]">
          {rowTop.map((item, idx) => (
            <span key={idx} className={item.tone}>
              {item.ch}
            </span>
          ))}
        </div>
        <div className="flex items-center justify-between gap-2 text-[9px]">
          <span
            className={cn(
              "tracking-[1.5px] opacity-75",
              armed ? "text-cyan" : captureActive ? "text-warn" : "text-ink-faint",
            )}
          >
            {rowBottom.slice(0, 18)}
          </span>
          <span
            className={cn(
              "font-semibold tracking-wider uppercase",
              armed ? "text-accent" : captureActive ? "text-warn" : "text-ink-dim",
            )}
          >
            {armed
              ? `VEIL_ACTIVE [${protectedCount}/${totalWindows}]`
              : captureActive
                ? "CAPTURE_PROC_DETECTED"
                : `STANDBY [${protectedCount} RULES]`}
          </span>
        </div>
      </div>
    </div>
  );
}

/**
 * Interactive 2D Desktop Spatial Radar & ASCII Veil Compositor Minimap.
 * Plots every live window proportionally and lets the user click any window to toggle protection.
 */
export function DesktopSpatialRadar({
  inventory,
  profile,
  onSelectWindow,
}: {
  inventory: Inventory;
  profile: Profile | null;
  onSelectWindow: (window: WindowInfo) => void;
}) {
  const [tick, setTick] = useState(0);

  useEffect(() => {
    const id = window.setInterval(() => {
      setTick((t) => (t + 1) % 64);
    }, 180);
    return () => window.clearInterval(id);
  }, []);

  const { desktopW, desktopH } = useMemo(() => {
    let maxW = 1280;
    let maxH = 800;
    for (const w of inventory.windows) {
      maxW = Math.max(maxW, w.bounds.x + w.bounds.w + 40);
      maxH = Math.max(maxH, w.bounds.y + w.bounds.h + 40);
    }
    return { desktopW: maxW, desktopH: maxH };
  }, [inventory.windows]);

  const asciiVeilNoise = useMemo(() => {
    const patterns = [
      "░▒▓█ VEILED █▓▒░",
      "▒▓█░ VEILED ░█▓▒",
      "▓█░▒ VEILED ▒░█▓",
      "█░▒▓ VEILED ▓▒░█",
    ];
    return patterns[tick % patterns.length];
  }, [tick]);

  return (
    <div className="relative overflow-hidden rounded-lg border border-line bg-void/90 p-2.5">
      <div className="mb-2 flex items-center justify-between font-mono text-[10px] text-ink-faint">
        <div className="flex items-center gap-2">
          <span className="inline-block size-1.5 rounded-full bg-accent animate-pulse" />
          <span className="font-semibold text-ink-dim">
            SPATIAL_DESKTOP_MAP // {desktopW}×{desktopH}px
          </span>
        </div>
        <div className="flex items-center gap-3">
          <span className="inline-flex items-center gap-1 text-good">
            <span>█</span> EXCLUDED
          </span>
          <span className="inline-flex items-center gap-1 text-accent">
            <span>▒</span> SURFACE_VEIL
          </span>
          <span className="inline-flex items-center gap-1 text-warn">
            <span>░</span> STRICT_HIDE
          </span>
          <span className="text-ink-faint">CLICK WINDOW TO TOGGLE</span>
        </div>
      </div>

      <div className="relative h-36 w-full overflow-hidden rounded border border-line-soft bg-radar-grid">
        {/* Subtle animated radar sweep beam */}
        <div
          aria-hidden="true"
          className="pointer-events-none absolute inset-x-0 h-8 bg-gradient-to-b from-transparent via-accent/[0.06] to-transparent animate-radar-sweep"
        />

        {/* Corner ASCII reticles */}
        <span
          aria-hidden="true"
          className="pointer-events-none absolute top-1 left-1.5 font-mono text-[9px] text-ink-faint/60"
        >
          ┌ 0,0
        </span>
        <span
          aria-hidden="true"
          className="pointer-events-none absolute top-1 right-1.5 font-mono text-[9px] text-ink-faint/60"
        >
          {desktopW},0 ┐
        </span>
        <span
          aria-hidden="true"
          className="pointer-events-none absolute bottom-1 left-1.5 font-mono text-[9px] text-ink-faint/60"
        >
          └ 0,{desktopH}
        </span>
        <span
          aria-hidden="true"
          className="pointer-events-none absolute bottom-1 right-1.5 font-mono text-[9px] text-ink-faint/60"
        >
          {desktopW},{desktopH} ┘
        </span>

        {inventory.windows.length === 0 && (
          <div className="flex h-full items-center justify-center font-mono text-xs text-ink-faint">
            [ NO_ACTIVE_WINDOWS_IN_VIEWPORT ]
          </div>
        )}

        {inventory.windows.map((w) => {
          const rule = findRuleForWindow(profile, w);
          const applied = inventory.applied.find((a) => a.window === w.id);
          const isProtected = Boolean(rule);
          const activeMode: ProtectionMode | null = applied?.mode ?? rule?.mode ?? null;
          const isCapture = isCaptureWindow(w);

          const leftPct = Math.max(1, Math.min(90, (Math.max(0, w.bounds.x) / desktopW) * 100));
          const topPct = Math.max(4, Math.min(72, (Math.max(0, w.bounds.y) / desktopH) * 100));
          const widthPct = Math.max(18, Math.min(99 - leftPct, (w.bounds.w / desktopW) * 100));
          const heightPct = Math.max(26, Math.min(96 - topPct, (w.bounds.h / desktopH) * 100));

          const modeColor =
            activeMode === "exclusion"
              ? "border-good bg-good/15 text-good shadow-[0_0_12px_rgba(16,185,129,0.25)]"
              : activeMode === "surface"
                ? "border-accent bg-accent/15 text-accent shadow-[0_0_12px_rgba(0,245,212,0.22)]"
                : activeMode === "strict_hide"
                  ? "border-warn bg-warn/15 text-warn shadow-[0_0_12px_rgba(245,158,11,0.22)]"
                  : isCapture
                    ? "border-bad/60 bg-bad/10 text-bad hover:border-bad"
                    : "border-line bg-panel-raised/90 text-ink-dim hover:border-cyan/60 hover:text-ink";

          return (
            <button
              key={w.id}
              type="button"
              onClick={() => onSelectWindow(w)}
              style={{
                left: `${leftPct}%`,
                top: `${topPct}%`,
                width: `${widthPct}%`,
                height: `${heightPct}%`,
              }}
              title={`${w.app_name} — ${w.title || "Untitled"} (${w.bounds.w}×${w.bounds.h}) — Click to toggle protection`}
              className={cn(
                "group absolute flex flex-col justify-between overflow-hidden rounded border px-2 py-1 text-left font-mono transition-all duration-150 focus-visible:ring-2 focus-visible:ring-accent cursor-pointer",
                modeColor,
                isProtected && "bg-veil-pattern",
              )}
            >
              <div className="flex items-center justify-between gap-1 text-[9px] leading-none">
                <span className="truncate font-bold uppercase text-ink">
                  {isProtected ? "◈ " : "▫ "}
                  {w.app_name}
                </span>
                <span className="shrink-0 text-cyan opacity-85">
                  0x{w.id.toString(16)}
                </span>
              </div>

              <div className="flex items-center justify-between gap-1 text-[8px] leading-none">
                <span className="truncate opacity-80">
                  {isProtected && activeMode ? asciiVeilNoise : `${w.bounds.w}×${w.bounds.h}`}
                </span>
                <span className="shrink-0 font-bold uppercase">
                  {activeMode ? modeLabel(activeMode) : isCapture ? "REC_APP" : "EXPOSED"}
                </span>
              </div>
            </button>
          );
        })}
      </div>
    </div>
  );
}

const CANARY_PALETTE = [
  "#00f5d4",
  "#ec4899",
  "#f59e0b",
  "#10b981",
  "#38bdf8",
  "#a855f7",
  "#f43f5e",
  "#eab308",
];

/**
 * Deterministic 8x8 Cryptographic Canary Token Matrix Visualizer.
 */
export function CanaryMatrixVisualizer({
  verification,
  busy,
}: {
  verification: VerificationRun | null;
  busy: boolean;
}) {
  const [scanRow, setScanRow] = useState(0);

  useEffect(() => {
    const id = window.setInterval(() => {
      setScanRow((r) => (r + 1) % 8);
    }, busy ? 80 : 320);
    return () => window.clearInterval(id);
  }, [busy]);

  const cells = useMemo(() => {
    const out: string[] = [];
    for (let y = 0; y < 8; y++) {
      for (let x = 0; x < 8; x++) {
        const hash = (x * 37 + y * 13 + (x ^ y) * 7) % CANARY_PALETTE.length;
        out.push(CANARY_PALETTE[hash]);
      }
    }
    return out;
  }, []);

  const afterMasked = verification ? !verification.after.found : false;

  return (
    <div className="grid grid-cols-2 gap-2.5 rounded-lg border border-line-soft bg-void/80 p-2.5 font-mono">
      {/* Baseline 8x8 Matrix */}
      <div className="flex flex-col gap-1.5">
        <div className="flex items-center justify-between text-[10px]">
          <span className="text-ink-dim font-semibold">01 // BASELINE_8x8</span>
          <span className="text-warn">
            {verification ? `${verification.baseline.matched}/64` : "NOT RUN"}
          </span>
        </div>
        <div className="grid grid-cols-8 gap-0.5 rounded border border-line bg-panel p-1.5">
          {cells.map((color, idx) => {
            const row = Math.floor(idx / 8);
            const activeScan = row === scanRow;
            return (
              <div
                key={idx}
                style={{ backgroundColor: color }}
                className={cn(
                  "aspect-square rounded-[2px] transition-opacity duration-150",
                  activeScan ? "opacity-100 ring-1 ring-white/60" : "opacity-80",
                )}
              />
            );
          })}
        </div>
        <span className="text-[9px] text-ink-faint">
          {verification?.baseline.origin
            ? `TOKEN LOCK @ ${verification.baseline.origin.x},${verification.baseline.origin.y}`
            : "CANARY INJECTED IN REGION"}
        </span>
      </div>

      {/* Post-Arm 8x8 Audit Matrix */}
      <div className="flex flex-col gap-1.5">
        <div className="flex items-center justify-between text-[10px]">
          <span className="text-ink-dim font-semibold">02 // POST_ARM_AUDIT</span>
          <span className={afterMasked ? "text-good font-bold" : "text-bad font-bold"}>
            {verification ? `${verification.after.matched}/64` : "NOT RUN"}
          </span>
        </div>
        <div className="grid grid-cols-8 gap-0.5 rounded border border-line bg-panel p-1.5">
          {cells.map((color, idx) => {
            const row = Math.floor(idx / 8);
            const col = idx % 8;
            const checker = (row + col) % 2 === 0;
            const activeScan = row === scanRow;
            return (
              <div
                key={idx}
                style={{
                  backgroundColor: afterMasked
                    ? checker
                      ? "#141c2b"
                      : "#0b0f17"
                    : color,
                }}
                className={cn(
                  " flex items-center justify-center aspect-square rounded-[2px] text-[7px] leading-none select-none transition-all",
                  afterMasked ? "text-accent/70" : "text-white",
                  activeScan && "ring-1 ring-accent/60 bg-accent/20",
                )}
              >
                {afterMasked ? (checker ? "░" : "▒") : "!"}
              </div>
            );
          })}
        </div>
        <span className={cn("text-[9px]", afterMasked ? "text-good" : "text-bad")}>
          {!verification ? "ILLUSTRATION ONLY" : afterMasked ? "✓ SYNTHETIC TOKEN ABSENT" : "✗ SYNTHETIC TOKEN OBSERVED"}
        </span>
      </div>
    </div>
  );
}
