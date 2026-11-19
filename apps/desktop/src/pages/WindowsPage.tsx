import { useEffect, useMemo, useState } from "react";
import * as api from "../api";
import { ProtectionModesGuide } from "../components/ProtectionModesGuide";
import {
  Map as MapIcon,
  Monitor,
  Radio,
  Search,
  ShieldAlert,
  ShieldCheck,
  SlidersHorizontal,
} from "lucide-react";
import type {
  AppliedProtection,
  Capabilities,
  Display,
  Inventory,
  Profile,
  ProtectionMode,
  WindowInfo,
} from "../types";
import { MODES, findRuleForWindow, isCaptureWindow, modeLabel, normalizeAppKey } from "../types";
import { cn } from "../lib/utils";
import { AppGlyph } from "../components/icons";
import { SpatialRadar } from "../components/Visuals";
import { Badge } from "../components/ui/badge";
import { Switch } from "../components/ui/switch";

type Filter = "all" | "protected" | "exposed" | "capture";

function preferredMode(caps: Capabilities | null): ProtectionMode {
  if (!caps) return "surface";
  for (const mode of ["exclusion", "surface", "strict_hide"] as ProtectionMode[]) {
    if (caps.modes[mode].level !== "unsupported") return mode;
  }
  return "strict_hide";
}

function StatusCell({ win, applied, configured }: { win: WindowInfo; applied?: AppliedProtection; configured: boolean }) {
  if (!applied) {
    return (
      <span className="inline-flex items-center gap-1.5 text-[11px] text-faint">
        <span className="size-1.5 rounded-full bg-border-strong" /> {configured ? "Configured" : win.visible ? "No rule" : "Off-screen"}
      </span>
    );
  }
  if (applied.status.status === "failed") {
    return (
      <span className="inline-flex items-center gap-1.5 text-[11px] text-danger" title={applied.status.reason}>
        <ShieldAlert className="size-3" /> Failed
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1.5 text-[11px] text-success">
      <ShieldCheck className="size-3" /> {modeLabel(applied.mode)}
    </span>
  );
}

export function WindowsPage({
  inventory,
  profile,
  caps,
  busy,
  onRefresh,
  onToggle,
  onMode,
}: {
  inventory: Inventory;
  profile: Profile | null;
  caps: Capabilities | null;
  busy: boolean;
  onRefresh: () => void;
  onToggle: (window: WindowInfo) => void;
  onMode: (window: WindowInfo, mode: ProtectionMode) => void;
}) {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [showRadar, setShowRadar] = useState(true);
  const [displays, setDisplays] = useState<Display[]>([]);
  useEffect(() => {
    const load = () => void api.surfaceDisplays().then(setDisplays, () => {});
    load();
    window.addEventListener("focus", load);
    return () => window.removeEventListener("focus", load);
  }, []);
  const [showUtility, setShowUtility] = useState(false);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const isMac = caps?.platform === "mac_os";
  const isUtility = (w: WindowInfo) => w.platform === "mac_os" && !w.title.trim() && w.bounds.w <= 80 && w.bounds.h <= 80;

  const counts = useMemo(() => {
    let protectedCount = 0;
    let captureCount = 0;
    for (const w of inventory.windows) {
      if (findRuleForWindow(profile, w)) protectedCount++;
      if (isCaptureWindow(w)) captureCount++;
    }
    return {
      all: inventory.windows.length,
      protected: protectedCount,
      exposed: inventory.windows.length - protectedCount,
      capture: captureCount,
    };
  }, [inventory.windows, profile]);

  const rows = useMemo(() => {
    const q = query.trim().toLowerCase();
    return inventory.windows
      .filter((w) => {
        if (!showUtility && isUtility(w) && !findRuleForWindow(profile, w)) return false;
        if (filter === "protected" && !findRuleForWindow(profile, w)) return false;
        if (filter === "exposed" && findRuleForWindow(profile, w)) return false;
        if (filter === "capture" && !isCaptureWindow(w)) return false;
        if (!q) return true;
        return (
          w.app_name.toLowerCase().includes(q) ||
          w.title.toLowerCase().includes(q) ||
          String(w.pid).includes(q) ||
          `0x${w.id.toString(16)}`.includes(q)
        );
      })
      .sort((a, b) => a.app_name.localeCompare(b.app_name) || a.title.localeCompare(b.title));
  }, [inventory.windows, profile, filter, query, showUtility]);

  const groups = new Map<string, WindowInfo[]>();
  for (const row of rows) {
    const key = isMac ? normalizeAppKey(row.app.value) : String(row.id);
    const group = groups.get(key) ?? [];
    group.push(row);
    groups.set(key, group);
  }

  return (
    <div className="animate-page-in space-y-4">
      {/* Summary row */}
      <div className="flex flex-wrap items-center gap-3">
        <div className="flex items-center gap-4 text-[12px]">
          <span className="text-muted">
            <strong className="font-semibold text-text">{counts.all}</strong> windows
          </span>
          <span className="text-muted">
          <strong className="font-semibold text-text">{counts.protected}</strong> configured
          </span>
          <span className="text-muted">
            <strong className="font-semibold text-text">{counts.exposed}</strong> without rules
          </span>
          {counts.capture > 0 && (
            <span className="text-muted">
              <strong className="font-semibold text-warning">{counts.capture}</strong> capture app
            </span>
          )}
        </div>
        <div className="ml-auto flex items-center gap-2">
          <button
            type="button"
            onClick={onRefresh}
            title="Refresh the list of running apps and windows"
            disabled={busy}
            className="h-8 rounded-md border border-border bg-panel px-2.5 text-[12px] text-muted transition-colors hover:border-border-strong hover:text-text disabled:opacity-50"
          >
            Refresh windows
          </button>
          <button
            type="button"
            onClick={() => setShowRadar((v) => !v)}
            title="Show or hide the desktop window layout"
            aria-expanded={showRadar}
            className={cn(
              "inline-flex h-8 items-center gap-1.5 rounded-md border px-2.5 text-[12px] transition-colors",
              showRadar
                ? "border-accent/40 bg-accent-soft text-accent"
                : "border-border bg-panel text-muted hover:text-text",
            )}
          >
            <MapIcon className="size-3.5" />
            Spatial map
          </button>
        </div>
      </div>

      {showRadar && (
        <SpatialRadar
          inventory={{ ...inventory, windows: inventory.windows.filter((w) => showUtility || !isUtility(w) || Boolean(findRuleForWindow(profile, w))) }}
          profile={profile}
          displays={displays}
          busy={busy}
          onSelectWindow={onToggle}
        />
      )}
      {isMac && <div className="flex flex-wrap items-center justify-between gap-2 text-xs text-muted">
        <span>Windows from the same app are grouped. App rules apply to every window in the group.</span>
        <label className="flex items-center gap-2" title="Include small, untitled utility windows in the list"><input type="checkbox" checked={showUtility} onChange={(e) => setShowUtility(e.target.checked)} /> Show small untitled windows</label>
      </div>}

      {/* Table */}
      <ProtectionModesGuide caps={caps} />
      <div className="overflow-hidden rounded-xl border border-border bg-panel">
        <div className="flex flex-wrap items-center gap-3 border-b border-border px-4 py-3">
          <div className="relative min-w-[220px] flex-1">
            <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-faint" />
            <input
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search app, title, PID, or window handle…"
              className="h-8 w-full rounded-md border border-border bg-app pl-8 pr-3 text-[12px] text-text outline-none placeholder:text-faint focus:border-accent/60"
            />
          </div>
          <div className="flex items-center gap-1 rounded-md border border-border bg-app p-0.5">
            {(
              [
                ["all", "All"],
                ["protected", "Configured"],
                ["exposed", "No rule"],
                ["capture", "Capture apps"],
              ] as [Filter, string][]
            ).map(([id, label]) => (
              <button
                key={id}
                type="button"
                onClick={() => setFilter(id)}
                className={cn(
                  "h-6 rounded px-2 text-[11px] transition-colors",
                  filter === id
                    ? "bg-elevated font-medium text-text shadow-sm"
                    : "text-faint hover:text-muted",
                )}
              >
                {label}
              </button>
            ))}
          </div>
          <SlidersHorizontal className="size-3.5 text-faint" />
        </div>

        <div className="grid grid-cols-[minmax(0,1fr)_160px_100px_70px] gap-3 border-b border-border bg-app/40 px-4 py-2 text-[10px] font-semibold uppercase tracking-[0.06em] text-faint">
          <span>Application / window</span>
          <span>Mode</span>
          <span>Status</span>
          <span className="text-right">Protect</span>
        </div>

        <div className="divide-y divide-border/70">
          {rows.length === 0 && (
            <div className="flex flex-col items-center gap-2 px-4 py-14 text-center">
              <div className="flex size-10 items-center justify-center rounded-lg border border-border bg-elevated text-faint">
                <Monitor className="size-4" />
              </div>
              <p className="text-[13px] font-medium text-text">
                {inventory.windows.length === 0 ? "No protectable windows" : "No matching windows"}
              </p>
              <p className="max-w-[320px] text-[12px] text-faint">
                {inventory.windows.length === 0
                  ? "Open an application and refresh. Protected windows stay usable locally while veiled for your audience."
                  : "Try clearing the search or switching filters."}
              </p>
            </div>
          )}
          {[...groups].map(([key, windows]) => {
            const win = windows.find((w) => w.visible) ?? windows[0];
            const rule = findRuleForWindow(profile, win);
            const enabled = Boolean(rule);
            const mode = rule?.mode ?? preferredMode(caps);
            const applied = inventory.applied.find((a) => a.window === win.id);
            return (
              <div key={key}>
              <div
                className={cn(
                  "grid grid-cols-[minmax(0,1fr)_160px_100px_70px] items-center gap-3 px-4 py-2.5 transition-colors hover:bg-hover/50",
                  enabled && "bg-accent-soft/[0.25]",
                )}
              >
                <div className="flex min-w-0 items-center gap-2.5">
                  <AppGlyph name={win.app_name} icon={win.icon} size={22} />
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="truncate text-[12px] font-medium text-text">{win.app_name}</span>
                      {isCaptureWindow(win) && (
                        <span title="Known capture application" className="text-warning">
                          <Radio className="size-3" />
                        </span>
                      )}
                    </div>
                    <div className="flex items-center gap-2 text-[11px] text-faint">
                      {windows.length > 1 ? <button type="button" className="underline hover:text-accent" title="List the individual windows from this app" aria-expanded={expanded.has(key)} onClick={() => setExpanded((old) => { const next = new Set(old); if (!next.delete(key)) next.add(key); return next; })}>{windows.length} windows</button> : <>
                        <span className="truncate">{win.title || "Untitled window"}</span>
                        <span className="shrink-0 font-mono">0x{win.id.toString(16)}</span>
                        <span className="hidden shrink-0 lg:inline">{win.bounds.w}×{win.bounds.h}</span>
                      </>}
                    </div>
                  </div>
                </div>

                <div>
                  <div className="inline-flex rounded-md border border-border bg-app p-0.5">
                    {MODES.map((entry) => {
                      const level = caps?.modes[entry.id];
                      const unsupported = level?.level === "unsupported";
                      const active = enabled && mode === entry.id;
                      return (
                        <button
                          key={entry.id}
                          type="button"
                          disabled={busy || unsupported}
                          onClick={() => onMode(win, entry.id)}
                          title={unsupported ? `${entry.label} is unsupported on this OS` : entry.blurb}
                          className={cn(
                            "h-6 rounded px-2 text-[10px] transition-all disabled:opacity-30",
                            active
                              ? entry.id === "exclusion"
                                ? "bg-success text-app font-semibold"
                                : entry.id === "surface"
                                  ? "bg-accent text-white font-semibold"
                                  : "bg-warning text-app font-semibold"
                              : "text-muted hover:bg-elevated hover:text-text",
                          )}
                        >
                          {entry.id === "exclusion" ? "Exclude" : entry.id === "surface" ? "Surface" : "Hide"}
                        </button>
                      );
                    })}
                  </div>
                </div>

                <div>
                  <StatusCell win={win} applied={applied} configured={enabled} />
                  {applied?.status.status === "failed" && (
                    <Badge variant="error" className="mt-1 text-[9px]">retrying</Badge>
                  )}
                </div>

                <div className="flex items-center justify-end gap-2">
                  <span className="hidden text-[10px] text-faint xl:inline">
                    {enabled ? rule?.target.kind === "window" ? "WIN" : "APP" : ""}
                  </span>
                  <Switch
                    checked={enabled}
                    disabled={busy}
                    aria-label={`Protect ${win.app_name}`}
                    title={enabled ? "Remove the protection rule" : "Configure protection for this app; use Arm to apply it"}
                    onCheckedChange={() => onToggle(win)}
                  />
                </div>
              </div>
              {windows.length > 1 && expanded.has(key) && <ul className="space-y-1 px-12 py-2 text-xs text-muted">
                {windows.map((w) => <li key={w.id} className="flex flex-wrap items-center gap-2">
                  <span>{w.title || "Untitled window"}</span><code className="text-faint">0x{w.id.toString(16)} · {w.bounds.w}×{w.bounds.h}</code>
                  <StatusCell win={w} applied={inventory.applied.find((item) => item.window === w.id)} configured={Boolean(findRuleForWindow(profile, w))} />
                </li>)}
              </ul>}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
