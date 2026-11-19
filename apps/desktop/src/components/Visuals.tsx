import { useEffect, useMemo, useState } from "react";
import type { Display, Inventory, Profile, WindowInfo } from "../types";
import { findRuleForWindow, isCaptureWindow, modeLabel } from "../types";
import { cn } from "../lib/utils";
import { AppGlyph } from "./icons";
import { DisplayPicker } from "./DisplayPicker";

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
   SpatialRadar — proportional window geometry and a separate inspector.
--------------------------------------------------------------------------- */
export function SpatialRadar({
  inventory,
  profile,
  displays,
  busy,
  onSelectWindow,
}: {
  inventory: Inventory;
  profile: Profile | null;
  displays: Display[];
  busy: boolean;
  onSelectWindow: (window: WindowInfo) => void;
}) {
  const [displayId, setDisplay] = useState<string | null>(null);
  const [selected, select] = useState<number | null>(null);
  const [zoom, setZoom] = useState(1);
  const [expanded, setExpanded] = useState(false);
  const colors = ["#3b82f6", "#10b981", "#f59e0b", "#a855f7", "#ec4899", "#06b6d4"];
  const display = displays.find((item) => item.id === displayId);
  const visible = inventory.windows.filter((w) => w.visible && !w.minimized);
  const layout = displays.length ? displays.map((d) => d.bounds) : visible.map((w) => w.bounds);
  const left = display?.bounds.x ?? Math.min(...layout.map((r) => r.x), ...(layout.length ? [] : [0]));
  const top = display?.bounds.y ?? Math.min(...layout.map((r) => r.y), ...(layout.length ? [] : [0]));
  const desktopW = display?.bounds.w ?? Math.max(1, ...(layout.length ? layout.map((r) => r.x + r.w - left) : [1280]));
  const desktopH = display?.bounds.h ?? Math.max(1, ...(layout.length ? layout.map((r) => r.y + r.h - top) : [800]));
  const edges = (r: WindowInfo["bounds"]) => {
    const x = Math.max(left, r.x), y = Math.max(top, r.y);
    const right = Math.min(left + desktopW, r.x + r.w), bottom = Math.min(top + desktopH, r.y + r.h);
    if (right <= x || bottom <= y) return null;
    return { left: `${(x - left) / desktopW * 100}%`, top: `${(y - top) / desktopH * 100}%`, width: `${(right - x) / desktopW * 100}%`, height: `${(bottom - y) / desktopH * 100}%` };
  };
  const windows = visible.filter((w) => edges(w.bounds)).sort((a, b) => b.bounds.w * b.bounds.h - a.bounds.w * a.bounds.h);
  const selection = windows.find((w) => w.id === selected);
  const selectionRule = selection && findRuleForWindow(profile, selection);

  return (
    <div className="overflow-hidden rounded-xl border border-border bg-app">
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border px-4 py-3 text-xs">
        <span className="flex items-center gap-1.5 text-muted">
          <span className="size-1.5 rounded-full bg-success animate-pulse-dot" />
          <strong className="text-text">Spatial Map</strong><span className="text-faint">· {desktopW}×{desktopH} · {windows.length} visible</span>
        </span>
        <span className="flex items-center gap-3 text-faint">
          <span className="text-success">■ excluded</span>
          <span className="text-accent">■ surface</span>
          <span className="text-warning">■ hide</span>
        </span>
        <div className="flex flex-wrap items-center gap-2">
          {displays.length > 0 && <DisplayPicker displays={displays} value={displayId} onChange={(value) => { setDisplay(value); setZoom(1); }} />}
          <button type="button" className="rounded border border-border px-2 py-1" title="Zoom out" aria-label="Zoom out" disabled={zoom <= 1} onClick={() => setZoom((z) => z - 0.25)}>−</button>
          <button type="button" className="rounded border border-border px-2 py-1" title="Reset zoom" onClick={() => setZoom(1)}>{Math.round(zoom * 100)}%</button>
          <button type="button" className="rounded border border-border px-2 py-1" title="Zoom in" aria-label="Zoom in" disabled={zoom >= 2.5} onClick={() => setZoom((z) => z + 0.25)}>+</button>
          <button type="button" className="rounded border border-border px-2 py-1" aria-pressed={expanded} onClick={() => setExpanded((v) => !v)}>{expanded ? "Fit panel" : "Taller map"}</button>
        </div>
      </div>
      {displays.length > 0 && <div className="flex flex-wrap gap-2 border-b border-border px-4 py-2" aria-label="Display legend">
        {displays.map((d, index) => <button type="button" key={d.id} aria-pressed={d.id === displayId} onClick={() => { setDisplay(d.id === displayId ? null : d.id); setZoom(1); }} className="inline-flex items-center gap-2 rounded-md border border-border bg-panel px-2 py-1 text-xs text-text hover:border-accent" title={`${d.name} · ${d.bounds.w}×${d.bounds.h}`}>
          <span className="size-2 rounded-full" style={{ backgroundColor: colors[index % colors.length] }} /> Display {index + 1} <span className="text-faint">{d.name}</span>
        </button>)}
      </div>}
      <div className={cn("spatial-dots overflow-auto p-4 [container-type:size]", expanded ? "h-[min(72vh,790px)]" : "h-[clamp(320px,48vh,570px)]")}>
        <div className="relative mx-auto border border-border-strong bg-sidebar/65 shadow-inner" style={{ aspectRatio: `${desktopW} / ${desktopH}`, width: `${zoom * 100}%`, maxWidth: `${zoom * 100 * desktopW / desktopH}cqh` }}>
        {displays.map((d, index) => <div key={d.id} style={{ ...(edges(d.bounds) ?? { display: "none" }), borderColor: colors[index % colors.length] }} className="pointer-events-none absolute overflow-hidden rounded-sm border-2 p-1.5"><span className="relative z-10 whitespace-nowrap rounded bg-panel px-1.5 py-0.5 text-[10px] font-semibold text-text">Display {index + 1}</span></div>)}
        {windows.length === 0 && <div className="absolute inset-0 grid place-items-center text-xs text-muted">No visible windows in this view</div>}
        {windows.map((win) => {
          const rule = findRuleForWindow(profile, win);
          const applied = inventory.applied.find((a) => a.window === win.id);
          const mode = applied?.mode ?? rule?.mode ?? null;

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
              onClick={() => select(win.id)}
              style={edges(win.bounds) ?? undefined}
              title={`${win.app_name} — ${win.title || "Untitled"} · ${win.bounds.w}×${win.bounds.h}`}
              aria-pressed={selected === win.id}
              className={cn(
                "absolute flex min-h-[3px] min-w-[3px] flex-col overflow-hidden rounded border px-1.5 py-1 text-left transition-colors hover:z-20 focus-visible:z-30",
                selected === win.id && "z-10 ring-2 ring-accent ring-offset-1 ring-offset-app",
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
                  {applied?.status.status === "applied" ? "ACTIVE" : rule ? "RULE" : ""}
                </span>
              </div>
            </button>
          );
        })}
        </div>
      </div>
      <div className="flex flex-wrap items-center justify-between gap-2 border-t border-border px-4 py-3 text-xs text-muted">
        {selection ? <><div className="flex min-w-0 items-center gap-2"><AppGlyph name={selection.app_name} icon={selection.icon} /><span className="truncate"><strong className="text-text">{selection.app_name}</strong> · {selection.title || "Untitled"} · {selection.bounds.w}×{selection.bounds.h}</span></div>
          {selection.top_level ? <button type="button" disabled={busy} className="rounded border border-border px-2 py-1 hover:text-accent" onClick={() => onSelectWindow(selection)}>{selectionRule ? "Remove app rule" : "Add protection rule"}</button> : <span>Dashboard · controlled by Arm/Disarm</span>}</> : <span>Click a window to inspect it. Selecting a window doesn’t change protection.</span>}
      </div>
    </div>
  );
}
