import { useMemo, useState } from "react";
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
  Inventory,
  Profile,
  ProtectionMode,
  WindowInfo,
} from "../types";
import { MODES, findRuleForWindow, isCaptureWindow, modeLabel } from "../types";
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

function StatusCell({ win, applied }: { win: WindowInfo; applied?: AppliedProtection }) {
  if (!win.visible) {
    return (
      <span className="inline-flex items-center gap-1.5 text-[11px] text-warning">
        <span className="size-1.5 rounded-full bg-warning" /> Hidden
      </span>
    );
  }
  if (!applied) {
    return (
      <span className="inline-flex items-center gap-1.5 text-[11px] text-faint">
        <span className="size-1.5 rounded-full bg-border-strong" /> Exposed
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
  const [showRadar, setShowRadar] = useState(false);

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
  }, [inventory.windows, profile, filter, query]);

  return (
    <div className="animate-page-in space-y-4">
      {/* Summary row */}
      <div className="flex flex-wrap items-center gap-3">
        <div className="flex items-center gap-4 text-[12px]">
          <span className="text-muted">
            <strong className="font-semibold text-text">{counts.all}</strong> windows
          </span>
          <span className="text-muted">
            <strong className="font-semibold text-success">{counts.protected}</strong> protected
          </span>
          <span className="text-muted">
            <strong className="font-semibold text-text">{counts.exposed}</strong> exposed
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
            disabled={busy}
            className="h-8 rounded-md border border-border bg-panel px-2.5 text-[12px] text-muted transition-colors hover:border-border-strong hover:text-text disabled:opacity-50"
          >
            Refresh
          </button>
          <button
            type="button"
            onClick={() => setShowRadar((v) => !v)}
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
          inventory={inventory}
          profile={profile}
          onSelectWindow={onToggle}
        />
      )}

      {/* Table */}
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
                ["protected", "Protected"],
                ["exposed", "Exposed"],
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

        <div className="grid grid-cols-[minmax(0,1.7fr)_minmax(0,1fr)_170px_120px] gap-3 border-b border-border bg-app/40 px-4 py-2 text-[10px] font-semibold uppercase tracking-[0.06em] text-faint max-lg:grid-cols-[minmax(0,1fr)_150px_100px]">
          <span>Application / window</span>
          <span className="max-lg:hidden">Mode</span>
          <span className="max-lg:hidden">Status</span>
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
          {rows.map((win) => {
            const rule = findRuleForWindow(profile, win);
            const enabled = Boolean(rule);
            const mode = rule?.mode ?? preferredMode(caps);
            const applied = inventory.applied.find((a) => a.window === win.id);
            return (
              <div
                key={win.id}
                className={cn(
                  "grid grid-cols-[minmax(0,1.7fr)_minmax(0,1fr)_170px_120px] items-center gap-3 px-4 py-2.5 transition-colors hover:bg-hover/50 max-lg:grid-cols-[minmax(0,1fr)_150px_100px]",
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
                      <span className="truncate">{win.title || "Untitled window"}</span>
                      <span className="shrink-0 font-mono">0x{win.id.toString(16)}</span>
                      <span className="hidden shrink-0 lg:inline">{win.bounds.w}×{win.bounds.h}</span>
                    </div>
                  </div>
                </div>

                <div className="max-lg:hidden">
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
                          title={unsupported ? `${entry.label} is unsupported on this OS` : `Use ${entry.label}`}
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

                <div className="max-lg:hidden">
                  <StatusCell win={win} applied={applied} />
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
                    onCheckedChange={() => onToggle(win)}
                  />
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
