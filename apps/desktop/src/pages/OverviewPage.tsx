import {
  ArrowRight,
  Layers3,
  Monitor,
  Radio,
  ShieldCheck,
  Sparkles,
  Target,
} from "lucide-react";
import type {
  BackendInfo,
  Capabilities,
  Inventory,
  Profile,
  SurfacePreview,
  WindowInfo,
} from "../types";
import { findRuleForWindow, isCaptureWindow, modeLabel } from "../types";
import { cn } from "../lib/utils";
import { AppGlyph } from "../components/icons";
import { ActivityStrip } from "../components/Visuals";

function StatCard({
  icon: Icon,
  label,
  value,
  hint,
  tone = "default",
}: {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  value: string | number;
  hint: string;
  tone?: "default" | "success" | "warning" | "accent";
}) {
  return (
    <div className="rounded-xl border border-border bg-panel p-4 transition-colors hover:border-border-strong">
      <div className="flex items-center justify-between">
        <span className="text-[11px] font-medium text-muted">{label}</span>
        <Icon
          className={cn(
            "size-4",
            tone === "success" && "text-success",
            tone === "warning" && "text-warning",
            tone === "accent" && "text-accent",
            tone === "default" && "text-faint",
          )}
        />
      </div>
      <p className="mt-2 text-[24px] font-semibold leading-none tracking-tight text-text">{value}</p>
      <p className="mt-1.5 text-[11px] text-faint">{hint}</p>
    </div>
  );
}

export function OverviewPage({
  inventory,
  profile,
  caps,
  preview,
  backend,
  onNavigate,
  onToggleWindow,
}: {
  inventory: Inventory;
  profile: Profile | null;
  caps: Capabilities | null;
  preview: SurfacePreview | null;
  backend: BackendInfo | null;
  onNavigate: (page: "windows" | "surface" | "audit" | "capabilities") => void;
  onToggleWindow: (window: WindowInfo) => void;
}) {
  const windows = inventory.windows;
  const protectedCount = windows.filter((w) => findRuleForWindow(profile, w)).length;
  const captureCount = windows.filter(isCaptureWindow).length;
  const armed = Boolean(inventory.armed) || inventory.applied.length > 0;
  const topWindows = windows.slice(0, 5);

  return (
    <div className="animate-page-in space-y-4">
      {/* Hero */}
      <div className="relative overflow-hidden rounded-xl border border-border bg-panel">
        <div className="hero-grid pointer-events-none absolute inset-0 opacity-40" />
        <div className="relative flex flex-wrap items-center justify-between gap-4 p-5">
          <div>
            <div className="flex items-center gap-2">
              <span
                className={cn(
                  "inline-flex size-2 rounded-full",
                  armed ? "bg-success animate-pulse-dot" : "bg-faint",
                )}
              />
              <span className="text-[11px] font-medium uppercase tracking-[0.08em] text-muted">
                {armed ? "Protection armed" : "Protection disarmed"}
              </span>
            </div>
            <h2 className="mt-2 text-[20px] font-semibold tracking-tight text-text text-balance">
              {armed
                ? `${protectedCount} window${protectedCount === 1 ? "" : "s"} veiled for your audience`
                : "Your desktop is currently unshielded"}
            </h2>
            <p className="mt-1 max-w-[520px] text-[12px] leading-relaxed text-muted">
              {armed
                ? "Protected windows stay usable on your screen while the shareable Surface shows a masked copy."
                : "Select the windows you want to protect, then arm the shield. Everything runs locally."}
            </p>
          </div>
          <ActivityStrip
            armed={armed}
            protectedCount={protectedCount}
            captureActive={captureCount > 0}
          />
        </div>
      </div>

      {/* Stats */}
      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        <StatCard
          icon={Monitor}
          label="Live windows"
          value={windows.length}
          hint="top-level windows detected"
        />
        <StatCard
          icon={ShieldCheck}
          label="Protected"
          value={protectedCount}
          hint="windows with an active rule"
          tone="success"
        />
        <StatCard
          icon={Radio}
          label="Capture apps"
          value={captureCount}
          hint={captureCount ? "recording tools detected" : "no recording tools detected"}
          tone={captureCount ? "warning" : "default"}
        />
        <StatCard
          icon={Layers3}
          label="Best mode"
          value={caps?.modes.exclusion.level !== "unsupported" ? "Exclusion" : "Surface"}
          hint={backend?.live ? `${backend.name} driver` : "preview mode"}
          tone="accent"
        />
      </div>

      {/* Main two-column */}
      <div className="grid gap-4 xl:grid-cols-[minmax(0,1.1fr)_minmax(0,0.9fr)]">
        {/* Live window list */}
        <div className="overflow-hidden rounded-xl border border-border bg-panel">
          <div className="flex items-center justify-between border-b border-border px-4 py-3">
            <div>
              <h3 className="text-[13px] font-semibold text-text">Target windows</h3>
              <p className="mt-0.5 text-[11px] text-faint">Toggle protection for any window</p>
            </div>
            <button
              type="button"
              onClick={() => onNavigate("windows")}
              className="inline-flex items-center gap-1 text-[11px] text-muted transition-colors hover:text-accent"
            >
              View all <ArrowRight className="size-3" />
            </button>
          </div>
          <div className="divide-y divide-border/70">
            {topWindows.length === 0 && (
              <div className="flex flex-col items-center gap-2 px-4 py-12 text-center">
                <Target className="size-5 text-faint" />
                <p className="text-[12px] font-medium text-text">No windows detected</p>
                <p className="max-w-[280px] text-[11px] text-faint">
                  Open an application to see it here. In browser preview, demo windows are shown.
                </p>
              </div>
            )}
            {topWindows.map((win) => {
              const rule = findRuleForWindow(profile, win);
              const applied = inventory.applied.find((a) => a.window === win.id);
              return (
                <button
                  key={win.id}
                  type="button"
                  onClick={() => onToggleWindow(win)}
                  className="flex w-full items-center gap-3 px-4 py-2.5 text-left transition-colors hover:bg-hover/60"
                >
                  <AppGlyph name={win.app_name} icon={win.icon} size={20} />
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-[12px] font-medium text-text">{win.app_name}</p>
                    <p className="truncate text-[11px] text-faint">{win.title || "Untitled window"}</p>
                  </div>
                  {isCaptureWindow(win) && <Radio className="size-3.5 shrink-0 text-warning" />}
                  {rule ? (
                    <span className="shrink-0 rounded border border-success/25 bg-success-soft px-1.5 py-0.5 text-[10px] text-success">
                      {modeLabel(applied?.mode ?? rule.mode)}
                    </span>
                  ) : (
                    <span className="shrink-0 text-[10px] text-faint">Exposed</span>
                  )}
                </button>
              );
            })}
          </div>
        </div>

        {/* Surface preview */}
        <div className="overflow-hidden rounded-xl border border-border bg-panel">
          <div className="flex items-center justify-between border-b border-border px-4 py-3">
            <div>
              <h3 className="text-[13px] font-semibold text-text">Veil Surface</h3>
              <p className="mt-0.5 text-[11px] text-faint">
                {preview ? `${preview.masked} masked · ${preview.source}` : "No frame yet"}
              </p>
            </div>
            <button
              type="button"
              onClick={() => onNavigate("surface")}
              className="inline-flex items-center gap-1 text-[11px] text-muted transition-colors hover:text-accent"
            >
              Open <ArrowRight className="size-3" />
            </button>
          </div>
          <div className="scan-overlay relative aspect-[16/10] bg-app">
            {preview?.png_base64 ? (
              <img
                src={`data:image/png;base64,${preview.png_base64}`}
                alt="Veil Surface preview"
                className="size-full object-contain"
              />
            ) : (
              <div className="flex h-full items-center justify-center font-mono text-[11px] text-faint">
                [ NO FRAME ]
              </div>
            )}
          </div>
          <div className="flex items-center gap-2 border-t border-border px-4 py-2.5 text-[11px] text-muted">
            <Sparkles className="size-3.5 text-info" />
            Share the Surface window, not your whole desktop.
          </div>
        </div>
      </div>
    </div>
  );
}
