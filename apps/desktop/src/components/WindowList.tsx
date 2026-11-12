import { useMemo, useState } from "react";
import { ChevronDown, Monitor, RefreshCw } from "lucide-react";
import type { Capabilities, Inventory, Profile, ProtectionMode, WindowInfo } from "../types";
import { MODES, modeLabel } from "../types";
import { cn } from "../lib/utils";
import { AppGlyph } from "./icons";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./ui/select";
import { Switch } from "./ui/switch";

function normalizeApp(value: string): string {
  return value.replaceAll("\\", "/").toLowerCase();
}

function ruleFor(profile: Profile | null, window: WindowInfo) {
  return profile?.rules.find(
    (rule) =>
      rule.enabled &&
      rule.target.kind === "app" &&
      normalizeApp(rule.target.value.value) === normalizeApp(window.app.value),
  );
}

function preferredMode(caps: Capabilities | null): ProtectionMode {
  if (!caps) return "surface";
  for (const mode of ["exclusion", "surface", "strict_hide"] as ProtectionMode[]) {
    if (caps.modes[mode].level !== "unsupported") return mode;
  }
  return "strict_hide";
}

function WindowRow({
  window,
  enabled,
  mode,
  live,
  caps,
  busy,
  onToggle,
  onMode,
}: {
  window: WindowInfo;
  enabled: boolean;
  mode: ProtectionMode;
  live?: { mode: ProtectionMode; status: { status: string } };
  caps: Capabilities | null;
  busy: boolean;
  onToggle: () => void;
  onMode: (mode: ProtectionMode) => void;
}) {
  return (
    <div
      className={cn(
        "group flex items-center gap-3 rounded-lg border border-transparent px-2.5 py-2 transition-colors",
        enabled ? "border-accent/20 bg-accent/[0.06]" : "hover:bg-row",
      )}
    >
      <AppGlyph name={window.app_name} />
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline gap-2">
          <span className="truncate text-[13px] font-medium text-ink">{window.app_name}</span>
          {window.title && (
            <span className="truncate text-xs text-ink-faint">{window.title}</span>
          )}
        </div>
        <div className="mt-0.5 flex items-center gap-2 text-[11px] text-ink-faint">
          <span className="font-mono">pid {window.pid}</span>
          <span className="inline-flex items-center gap-1">
            <Monitor className="size-3" />
            {window.bounds.w}×{window.bounds.h}
          </span>
          {!window.visible && <Badge variant="default">hidden</Badge>}
          {window.minimized && <Badge variant="default">minimized</Badge>}
        </div>
      </div>

      {enabled && live && live.status.status !== "applied" && (
        <Badge variant="error">{live.status.status}</Badge>
      )}
      {enabled && live && live.status.status === "applied" && (
        <Badge variant="accent">{modeLabel(live.mode)}</Badge>
      )}

      <Select
        value={mode}
        disabled={busy || !enabled}
        onValueChange={(value) => onMode(value as ProtectionMode)}
      >
        <SelectTrigger className="h-8 w-36 text-xs" aria-label="Protection mode">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {MODES.map((entry) => {
            const level = caps?.modes[entry.id];
            const unsupported = level?.level === "unsupported";
            return (
              <SelectItem key={entry.id} value={entry.id} disabled={unsupported}>
                {entry.label}
                {unsupported ? " —" : level?.level === "partial" ? " ◐" : ""}
              </SelectItem>
            );
          })}
        </SelectContent>
      </Select>

      <Switch
        checked={enabled}
        disabled={busy}
        aria-label={`Protect ${window.app_name}`}
        onCheckedChange={onToggle}
      />
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
  const allProtected = windows.every((w) => ruleFor(profile, w));
  const anyProtected = windows.some((w) => ruleFor(profile, w));

  return (
    <div className="rounded-xl border border-line-soft bg-panel-raised/40">
      <button
        className="flex w-full items-center gap-3 px-3 py-2.5 text-left outline-none focus-visible:ring-2 focus-visible:ring-accent/60"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
      >
        <ChevronDown
          className={cn("size-4 text-ink-faint transition-transform", !open && "-rotate-90")}
        />
        <AppGlyph name={appName} size={20} />
        <span className="flex-1 truncate text-[13px] font-semibold text-ink">{appName}</span>
        <span className="text-[11px] text-ink-faint">
          {windows.length} window{windows.length === 1 ? "" : "s"}
        </span>
        <span
          className={cn(
            "size-2 rounded-full",
            allProtected ? "bg-accent" : anyProtected ? "bg-warn" : "bg-line",
          )}
        />
      </button>
      {open && (
        <div className="space-y-0.5 px-1.5 pb-2">
          {windows.map((window) => {
            const rule = ruleFor(profile, window);
            const enabled = Boolean(rule);
            const mode = rule?.mode ?? preferredMode(caps);
            const live = inventory.applied.find((entry) => entry.window === window.id);
            return (
              <WindowRow
                key={window.id}
                window={window}
                enabled={enabled}
                mode={mode}
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
  const groups = useMemo(() => {
    const map = new Map<string, WindowInfo[]>();
    for (const window of inventory.windows) {
      const list = map.get(window.app_name) ?? [];
      list.push(window);
      map.set(window.app_name, list);
    }
    return [...map.entries()].sort((a, b) => a[0].localeCompare(b[0]));
  }, [inventory.windows]);

  const protectedCount = inventory.windows.filter((w) => ruleFor(profile, w)).length;

  return (
    <section className="flex min-h-0 flex-1 flex-col rounded-xl border border-line bg-panel">
      <div className="flex items-center justify-between gap-3 border-b border-line-soft px-4 py-3">
        <div>
          <h2 className="text-sm font-semibold tracking-tight">Windows</h2>
          <p className="mt-0.5 text-xs text-ink-faint">
            {inventory.windows.length} top-level · {protectedCount} protected
          </p>
        </div>
        <Button variant="ghost" size="sm" onClick={onRefresh} disabled={busy}>
          <RefreshCw className={cn("size-3.5", busy && "animate-spin")} />
          Refresh
        </Button>
      </div>

      <div className="min-h-0 flex-1 space-y-1.5 overflow-y-auto p-2.5">
        {groups.length === 0 && (
          <div className="flex h-full flex-col items-center justify-center gap-2 text-center">
            <p className="text-sm font-medium text-ink-dim">No protectable windows</p>
            <p className="max-w-64 text-xs leading-relaxed text-ink-faint">
              Start an application, then refresh. Protected windows stay visible to you and are
              masked for your audience.
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
