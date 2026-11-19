import { useEffect, useMemo, useState } from "react";
import type { Inventory, Profile, WindowInfo } from "../types";
import { findRuleForWindow, isCaptureWindow, modeLabel } from "../types";
import { cn } from "../lib/utils";

/* ---------------------------------------------------------------------------
   ActivityStrip — a quiet, monospace protection telemetry readout.
--------------------------------------------------------------------------- */
const WAVE = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█", "▇", "▆", "▅", "▄", "▃", "▂"];

export function ActivityStrip({
  armed,
  protectedCount,
  captureActive,
  compact = false,
}: {
  armed: boolean;
  protectedCount: number;
  captureActive: boolean;
  compact?: boolean;
}) {
  const [tick, setTick] = useState(0);
  useEffect(() => {
    const id = window.setInterval(() => setTick((t) => t + 1), 140);
    return () => window.clearInterval(id);
  }, []);

  const bars = useMemo(
    () =>
      Array.from({ length: 26 }, (_, i) => {
        const phase = (i + tick) % WAVE.length;
        return armed ? WAVE[phase] : captureActive ? (phase % 2 ? "▃" : "▁") : "▁";
      }),
    [armed, captureActive, tick],
  );

  return (
    <div
      className={cn(
        "flex items-center gap-2.5 rounded-md border px-2.5 py-1 font-mono text-[10px]",
        armed
          ? "border-success/25 bg-success-soft"
          : captureActive
            ? "border-warning/25 bg-warning-soft"
            : "border-border bg-panel",
      )}
    >
      <span
        className={cn(
          "size-1.5 shrink-0 rounded-full",
          armed ? "bg-success animate-pulse-dot" : captureActive ? "bg-warning" : "bg-faint",
        )}
      />
      <span
        className={cn(
          "tracking-[0.12em]",
          armed ? "text-success" : captureActive ? "text-warning" : "text-faint",
        )}
      >
        {bars.join("")}
      </span>
      {!compact && (
        <span className={cn("shrink-0", armed ? "text-success" : "text-faint")}>
          {armed ? `VEIL ACTIVE · ${protectedCount}` : captureActive ? "CAPTURE APP DETECTED" : "STANDBY"}
        </span>
      )}
    </div>
  );
}

/* ---------------------------------------------------------------------------
   SpatialRadar — proportional 2D map of live windows. Click to toggle.
--------------------------------------------------------------------------- */
export function SpatialRadar({
  inventory,
  profile,
  onSelectWindow,
}: {
  inventory: Inventory;
  profile: Profile | null;
  onSelectWindow: (window: WindowInfo) => void;
}) {
  const { w: desktopW, h: desktopH } = useMemo(() => {
    let maxW = 1280;
    let maxH = 800;
    for (const win of inventory.windows) {
      maxW = Math.max(maxW, win.bounds.x + win.bounds.w + 40);
      maxH = Math.max(maxH, win.bounds.y + win.bounds.h + 40);
    }
    return { w: maxW, h: maxH };
  }, [inventory.windows]);

  return (
    <div className="overflow-hidden rounded-lg border border-border bg-app">
      <div className="flex items-center justify-between border-b border-border px-3 py-2 font-mono text-[10px]">
        <span className="flex items-center gap-1.5 text-muted">
          <span className="size-1.5 rounded-full bg-success animate-pulse-dot" />
          SPATIAL MAP <span className="text-faint">· {desktopW}×{desktopH}</span>
        </span>
        <span className="flex items-center gap-3 text-faint">
          <span className="text-success">■ excluded</span>
          <span className="text-accent">■ surface</span>
          <span className="text-warning">■ hide</span>
          <span className="hidden sm:inline">click a window to toggle</span>
        </span>
      </div>
      <div className="hero-grid scan-overlay relative h-40 w-full">
        {inventory.windows.length === 0 && (
          <div className="flex h-full items-center justify-center font-mono text-[11px] text-faint">
            [ NO WINDOWS IN VIEWPORT ]
          </div>
        )}
        {inventory.windows.map((win) => {
          const rule = findRuleForWindow(profile, win);
          const applied = inventory.applied.find((a) => a.window === win.id);
          const mode = applied?.mode ?? rule?.mode ?? null;
          const left = Math.max(1, Math.min(90, (Math.max(0, win.bounds.x) / desktopW) * 100));
          const top = Math.max(6, Math.min(66, (Math.max(0, win.bounds.y) / desktopH) * 100));
          const width = Math.max(16, Math.min(99 - left, (win.bounds.w / desktopW) * 100));
          const height = Math.max(26, Math.min(94 - top, (win.bounds.h / desktopH) * 100));

          const tone =
            mode === "exclusion"
              ? "border-success bg-success-soft text-success"
              : mode === "surface"
                ? "border-accent bg-accent-soft text-accent"
                : mode === "strict_hide"
                  ? "border-warning bg-warning-soft text-warning"
                  : isCaptureWindow(win)
                    ? "border-danger/50 bg-danger-soft text-danger"
                    : "border-border-strong bg-panel/90 text-muted hover:border-accent/60 hover:text-text";

          return (
            <button
              key={win.id}
              type="button"
              onClick={() => onSelectWindow(win)}
              style={{ left: `${left}%`, top: `${top}%`, width: `${width}%`, height: `${height}%` }}
              title={`${win.app_name} — ${win.title || "Untitled"} · click to toggle`}
              className={cn(
                "absolute flex flex-col justify-between overflow-hidden rounded border px-1.5 py-1 text-left font-mono transition-all hover:z-10",
                tone,
              )}
            >
              <div className="flex items-center justify-between gap-1 text-[9px] leading-none">
                <span className="truncate font-semibold text-text">{win.app_name}</span>
                <span className="shrink-0 opacity-80">0x{win.id.toString(16)}</span>
              </div>
              <div className="flex items-center justify-between gap-1 text-[8px] leading-none">
                <span className="truncate opacity-70">
                  {mode ? modeLabel(mode) : win.title || `${win.bounds.w}×${win.bounds.h}`}
                </span>
                <span className="shrink-0 font-semibold uppercase opacity-90">
                  {mode ? "VEILED" : "EXPOSED"}
                </span>
              </div>
            </button>
          );
        })}
      </div>
    </div>
  );
}
