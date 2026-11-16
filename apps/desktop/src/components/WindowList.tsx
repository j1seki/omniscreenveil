import { useMemo, useState } from "react";
import {
  ChevronDown,
  Eye,
  EyeOff,
  Layers,
  Map as MapIcon,
  Monitor,
  Radio,
  RefreshCw,
  Search,
  ShieldAlert,
  ShieldCheck,
} from "lucide-react";
import type {
  AppliedProtection,
  Capabilities,
  Inventory,
  Profile,
  ProtectionMode,
  WindowInfo,
} from "../types";
import {
  MODES,
  findRuleForWindow,
  isCaptureWindow,
  modeLabel,
} from "../types";
import { cn } from "../lib/utils";
import { DesktopSpatialRadar } from "./AsciiMotion";
import { AppGlyph } from "./icons";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Switch } from "./ui/switch";

type FilterTab = "all" | "protected" | "exposed" | "capture";

function preferredMode(caps: Capabilities | null): ProtectionMode {
  if (!caps) return "surface";
  for (const mode of ["exclusion", "surface", "strict_hide"] as ProtectionMode[]) {
    if (caps.modes[mode].level !== "unsupported") return mode;
  }
  return "strict_hide";
}

function WindowStateBadge({ window }: { window: WindowInfo }) {
  if (!window.visible) {
    return (
      <span className="inline-flex items-center gap-1 rounded border border-warn/40 bg-warn/10 px-1.5 py-0.5 font-mono text-[10px] text-warn">
        <EyeOff className="size-2.5" />
        HIDDEN
      </span>
    );
  }
  if (window.minimized) {
    return (
      <span className="inline-flex items-center gap-1 rounded border border-line bg-row px-1.5 py-0.5 font-mono text-[10px] text-ink-faint">
        MINIMIZED
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1 rounded border border-good/30 bg-good/10 px-1.5 py-0.5 font-mono text-[10px] text-good">
      <Eye className="size-2.5" />
      VISIBLE
    </span>
  );
}

function WindowRow({
  window,
  enabled,
  mode,
  ruleScope,
  live,
  caps,
  busy,
  onToggle,
  onMode,
}: {
  window: WindowInfo;
  enabled: boolean;
  mode: ProtectionMode;
  ruleScope: "app" | "window" | null;
  live?: AppliedProtection;
  caps: Capabilities | null;
  busy: boolean;
  onToggle: () => void;
  onMode: (mode: ProtectionMode) => void;
}) {
  const isCapture = isCaptureWindow(window);
  const failedReason =
    live?.status.status === "failed" ? live.status.reason : undefined;

  return (
    <div
      className={cn(
        "group flex flex-col gap-1.5 rounded-lg border px-3 py-2 transition-all",
        enabled
          ? mode === "exclusion"
            ? "border-good/35 bg-good/[0.06]"
            : mode === "surface"
              ? "border-accent/35 bg-accent/[0.06]"
              : "border-warn/35 bg-warn/[0.06]"
          : "border-line-soft/70 bg-void/40 hover:border-line hover:bg-row/60",
      )}
    >
      <div className="flex items-center gap-3">
        <AppGlyph name={window.app_name} icon={window.icon} size={22} />

        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="truncate font-mono text-xs font-bold text-ink">
              {window.app_name}
            </span>
            {window.title && (
              <span className="truncate text-xs text-ink-dim">
                — {window.title}
              </span>
            )}
            {isCapture && (
              <span className="inline-flex shrink-0 items-center gap-1 rounded border border-warn/40 bg-warn/15 px-1.5 py-0.5 font-mono text-[9px] font-bold text-warn uppercase">
                <Radio className="size-2.5" />
                CAPTURE_APP
              </span>
            )}
          </div>

          <div className="mt-1 flex flex-wrap items-center gap-2 font-mono text-[10px] text-ink-faint">
            <span className="rounded bg-panel px-1.5 py-0.5 text-cyan">
              0x{window.id.toString(16)}
            </span>
            <span>PID:{window.pid}</span>
            <span className="inline-flex items-center gap-1">
              <Monitor className="size-2.5" />
              {window.bounds.w}×{window.bounds.h} @{window.bounds.x},{window.bounds.y}
            </span>
            <WindowStateBadge window={window} />
            {enabled && ruleScope && (
              <span className="rounded border border-violet/30 bg-violet/10 px-1.5 py-0.5 text-[9px] text-violet uppercase">
                {ruleScope === "window" ? "WIN_RULE" : "APP_RULE"}
              </span>
            )}
            {enabled && live && live.status.status === "applied" && (
              <Badge variant="live" className="px-1.5 py-0 text-[9px] uppercase">
                <ShieldCheck className="size-2.5" />
                {modeLabel(live.mode)} APPLIED
              </Badge>
            )}
            {enabled && !live && (
              <span className="rounded border border-accent/30 bg-accent/10 px-1.5 py-0.5 text-[9px] text-accent uppercase">
                RULE QUEUED
              </span>
            )}
          </div>
        </div>

        {/* Direct Tactile Mode Segmented Control — always clickable for supported modes */}
        <div
          role="group"
          aria-label="Protection mode"
          className="flex items-center rounded-lg border border-line bg-void p-0.5 font-mono text-[10px]"
        >
          {MODES.map((entry) => {
            const level = caps?.modes[entry.id];
            const unsupported = level?.level === "unsupported";
            const active = enabled && mode === entry.id;
            const label =
              entry.id === "exclusion"
                ? "Exclude"
                : entry.id === "surface"
                  ? "Surface"
                  : "Hide";

            const activeClasses =
              entry.id === "exclusion"
                ? "bg-good text-void font-bold shadow-[0_0_10px_rgba(16,185,129,0.35)]"
                : entry.id === "surface"
                  ? "bg-accent text-void font-bold shadow-[0_0_10px_rgba(0,245,212,0.35)]"
                  : "bg-warn text-void font-bold shadow-[0_0_10px_rgba(245,158,11,0.35)]";

            return (
              <button
                key={entry.id}
                type="button"
                disabled={busy || unsupported}
                onClick={() => onMode(entry.id)}
                title={
                  unsupported
                    ? `${entry.label}: unsupported on this OS`
                    : `Protect with ${entry.label}`
                }
                className={cn(
                  "rounded-md px-2 py-1 transition-all cursor-pointer disabled:cursor-not-allowed disabled:opacity-35",
                  active
                    ? activeClasses
                    : "text-ink-dim hover:bg-row hover:text-ink",
                )}
              >
                {label}
                {unsupported ? " —" : level?.level === "partial" ? " ◐" : ""}
              </button>
            );
          })}
        </div>

        <Switch
          checked={enabled}
          disabled={busy}
          aria-label={`Protect ${window.app_name}`}
          onCheckedChange={onToggle}
        />
      </div>

      {failedReason && (
        <div className="flex items-center gap-2 rounded border border-bad/40 bg-bad/10 px-2.5 py-1 font-mono text-[10px] text-bad">
          <ShieldAlert className="size-3 shrink-0" />
          <span className="truncate">{failedReason}</span>
        </div>
      )}
    </div>
  );
}

function AppGroup({
  appName,
  windows,
  profile,
  caps,
  inventory,
  busy,
  onToggle,
  onMode,
}: {
  appName: string;
  windows: WindowInfo[];
  profile: Profile | null;
  caps: Capabilities | null;
  inventory: Inventory;
  busy: boolean;
  onToggle: (window: WindowInfo) => void;
  onMode: (window: WindowInfo, mode: ProtectionMode) => void;
}) {
  const [open, setOpen] = useState(true);
  const allProtected = windows.every((w) => findRuleForWindow(profile, w));
  const anyProtected = windows.some((w) => findRuleForWindow(profile, w));
  const hasCapture = windows.some(isCaptureWindow);

  return (
    <div
      className={cn(
        "rounded-xl border transition-colors",
        allProtected
          ? "border-accent/35 bg-panel-raised/70"
          : anyProtected
            ? "border-warn/35 bg-panel-raised/60"
            : "border-line-soft bg-panel-raised/40",
      )}
    >
      <button
        type="button"
        className="flex w-full items-center gap-2.5 px-3 py-2 text-left outline-none focus-visible:ring-2 focus-visible:ring-accent/60 cursor-pointer"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
      >
        <ChevronDown
          className={cn(
            "size-3.5 text-ink-faint transition-transform",
            !open && "-rotate-90",
          )}
        />
        <AppGlyph name={appName} icon={windows[0]?.icon} size={18} />
        <span className="font-mono text-xs font-bold tracking-tight text-ink">
          {appName}
        </span>
        {hasCapture && (
          <span className="rounded border border-warn/40 bg-warn/15 px-1.5 py-0.5 font-mono text-[9px] font-semibold text-warn">
            SCREEN_RECORDER
          </span>
        )}
        <span className="ml-auto font-mono text-[10px] text-ink-faint">
          [{windows.length} {windows.length === 1 ? "WIN" : "WINS"}]
        </span>
        <span
          className={cn(
            "size-2 rounded-full",
            allProtected
              ? "bg-accent shadow-[0_0_8px_rgba(0,245,212,0.8)]"
              : anyProtected
                ? "bg-warn"
                : "bg-line",
          )}
        />
      </button>

      {open && (
        <div className="space-y-1 px-2 pb-2">
          {windows.map((window) => {
            const rule = findRuleForWindow(profile, window);
            const enabled = Boolean(rule);
            const mode = rule?.mode ?? preferredMode(caps);
            const live = inventory.applied.find(
              (entry) => entry.window === window.id,
            );
            return (
              <WindowRow
                key={window.id}
                window={window}
                enabled={enabled}
                mode={mode}
                ruleScope={rule ? rule.target.kind : null}
                live={live}
                caps={caps}
                busy={busy}
                onToggle={() => onToggle(window)}
                onMode={(next) => onMode(window, next)}
              />
            );
          })}
        </div>
      )}
    </div>
  );
}

export function WindowList({
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
  const [filter, setFilter] = useState<FilterTab>("all");
  const [showRadar, setShowRadar] = useState(true);

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

  const filteredWindows = useMemo(() => {
    const q = query.trim().toLowerCase();
    return inventory.windows.filter((w) => {
      const isProtected = Boolean(findRuleForWindow(profile, w));
      if (filter === "protected" && !isProtected) return false;
      if (filter === "exposed" && isProtected) return false;
      if (filter === "capture" && !isCaptureWindow(w)) return false;
      if (!q) return true;
      const hex = `0x${w.id.toString(16)}`;
      return (
        w.app_name.toLowerCase().includes(q) ||
        w.title.toLowerCase().includes(q) ||
        String(w.pid).includes(q) ||
        hex.includes(q)
      );
    });
  }, [inventory.windows, profile, filter, query]);

  const groups = useMemo(() => {
    const map = new Map<string, WindowInfo[]>();
    for (const window of filteredWindows) {
      const list = map.get(window.app_name) ?? [];
      list.push(window);
      map.set(window.app_name, list);
    }
    return [...map.entries()].sort((a, b) => a[0].localeCompare(b[0]));
  }, [filteredWindows]);

  return (
    <section className="flex min-h-0 flex-1 flex-col rounded-xl border border-line bg-panel/95 shadow-lg">
      {/* Header Bar */}
      <div className="flex flex-wrap items-center justify-between gap-2 border-b border-line-soft px-3.5 py-2.5">
        <div className="flex items-center gap-2.5">
          <span className="rounded border border-accent/40 bg-accent/10 px-1.5 py-0.5 font-mono text-[10px] font-bold text-accent">
            01 // MATRIX
          </span>
          <div>
            <h2 className="font-mono text-xs font-bold tracking-wide text-ink uppercase">
              Target Window Inventory
            </h2>
            <p className="font-mono text-[10px] text-ink-faint">
              {counts.all} TOP-LEVEL · {counts.protected} VEILED · {counts.exposed} EXPOSED
            </p>
          </div>
        </div>

        <div className="flex items-center gap-1.5">
          <Button
            variant={showRadar ? "secondary" : "ghost"}
            size="sm"
            onClick={() => setShowRadar((v) => !v)}
            className="h-7 gap-1.5 font-mono text-[10px]"
          >
            <MapIcon className="size-3 text-accent" />
            RADAR: {showRadar ? "ON" : "OFF"}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={onRefresh}
            disabled={busy}
            className="h-7 gap-1.5 font-mono text-[10px]"
          >
            <RefreshCw className={cn("size-3", busy && "animate-spin")} />
            Refresh
          </Button>
        </div>
      </div>

      {/* Interactive Spatial Radar Map */}
      {showRadar && (
        <div className="border-b border-line-soft px-3 py-2">
          <DesktopSpatialRadar
            inventory={inventory}
            profile={profile}
            onSelectWindow={onToggle}
          />
        </div>
      )}

      {/* Search & Filter Strip */}
      <div className="flex flex-wrap items-center justify-between gap-2 border-b border-line-soft bg-void/40 px-3 py-2">
        <div className="relative min-w-44 flex-1">
          <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-ink-faint" />
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Filter app, window title, PID, or 0x handle…"
            className="h-7 w-full rounded-lg border border-line bg-panel pl-8 pr-2.5 font-mono text-[11px] text-ink placeholder:text-ink-faint focus:border-accent focus:outline-none"
          />
        </div>

        <div className="flex items-center gap-1 font-mono text-[10px]">
          {(
            [
              { id: "all", label: `ALL (${counts.all})` },
              { id: "protected", label: `VEILED (${counts.protected})` },
              { id: "exposed", label: `EXPOSED (${counts.exposed})` },
              { id: "capture", label: `REC (${counts.capture})` },
            ] as { id: FilterTab; label: string }[]
          ).map((tab) => (
            <button
              key={tab.id}
              type="button"
              onClick={() => setFilter(tab.id)}
              className={cn(
                "rounded-md border px-2 py-1 transition-colors cursor-pointer",
                filter === tab.id
                  ? "border-accent/50 bg-accent/15 font-bold text-accent"
                  : "border-transparent text-ink-faint hover:bg-row hover:text-ink",
              )}
            >
              {tab.label}
            </button>
          ))}
        </div>
      </div>

      {/* Window List Body */}
      <div className="min-h-0 flex-1 space-y-2 overflow-y-auto p-2.5">
        {groups.length === 0 && (
          <div className="flex h-full flex-col items-center justify-center gap-2 py-8 text-center font-mono">
            <Layers className="size-6 text-ink-faint" />
            <p className="text-xs font-bold text-ink-dim">
              {inventory.windows.length === 0
                ? "[ NO_PROTECTABLE_WINDOWS_FOUND ]"
                : "[ NO_WINDOWS_MATCH_FILTER ]"}
            </p>
            <p className="max-w-72 text-[11px] leading-relaxed text-ink-faint">
              {inventory.windows.length === 0
                ? "Open any application window and click Refresh. Protected windows remain interactive locally while veiled from your audience."
                : "Clear your search filter or switch tabs to inspect all top-level windows."}
            </p>
          </div>
        )}
        {groups.map(([appName, windows]) => (
          <AppGroup
            key={appName}
            appName={appName}
            windows={windows}
            profile={profile}
            caps={caps}
            inventory={inventory}
            busy={busy}
            onToggle={onToggle}
            onMode={onMode}
          />
        ))}
      </div>
    </section>
  );
}
