import { ChevronRight, Command, Radio, RefreshCw, Shield, ShieldCheck, Sparkles } from "lucide-react";
import type { BackendInfo, Platform } from "../types";
import { PLATFORM_LABELS } from "../types";
import { cn } from "../lib/utils";
import type { Page } from "./Sidebar";
import { Button } from "./ui/button";
import { Switch } from "./ui/switch";

export const MOD = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform) ? "⌘" : "Ctrl+";

const TITLES: Record<Page, { title: string; subtitle: string }> = {
  overview: { title: "Overview", subtitle: "Your capture privacy at a glance" },
  windows: { title: "Windows", subtitle: "Choose which windows are protected and how" },
  preview: { title: "Preview", subtitle: "Dashboard playback and Stream output settings" },
  audit: { title: "Test suite", subtitle: "Run live capture and protection checks" },
  rules: { title: "Rules & automation", subtitle: "Persistent protection policies and auto-arm" },
  capabilities: { title: "Capabilities", subtitle: "What each operating system can actually do" },
  updates: { title: "Updates", subtitle: "Stable and beta release channels" },
  settings: { title: "Settings", subtitle: "Startup, window behavior, and appearance" },
};

export function TitleBar({
  page,
  backend,
  platform,
  armed,
  busy,
  onOpenPalette,
  onRefresh,
  onToggleArm,
  streamEnabled,
  streamControlsDisabled,
  onToggleStream,
  onOpenStream,
}: {
  page: Page;
  backend: BackendInfo | null;
  platform: Platform | null;
  armed: boolean;
  busy: boolean;
  onOpenPalette: () => void;
  onRefresh: () => void;
  onToggleArm: () => void;
  streamEnabled: boolean;
  streamControlsDisabled: boolean;
  onToggleStream: (enabled: boolean) => void;
  onOpenStream: () => void;
}) {
  const meta = TITLES[page];
  const live = backend?.live ?? false;

  return (
    <header className="flex h-14 shrink-0 items-center justify-between gap-4 border-b border-border bg-app/95 px-6 backdrop-blur">
      <div className="flex min-w-0 items-center gap-2">
        <span className="text-[12px] text-faint">Workspace</span>
        <ChevronRight className="size-3 text-faint" />
        <h1 className="truncate text-[15px] font-semibold tracking-tight text-text">
          {meta.title}
        </h1>
        <span className="ml-2 hidden truncate text-[12px] text-faint 2xl:block">
          {meta.subtitle}
        </span>
      </div>

      <div className="flex shrink-0 items-center gap-2">
        <button
          type="button"
          onClick={onOpenPalette}
          title={`Search commands (${MOD}K)`}
          className="hidden h-8 items-center gap-2 rounded-md border border-border bg-panel px-2.5 text-[12px] text-faint transition-colors hover:border-border-strong hover:text-muted md:flex"
        >
          <Command className="size-3.5" />
          <span>Quick actions</span>
          <kbd className="font-mono text-[10px]">{MOD}K</kbd>
        </button>

        {platform && (
          <span className="hidden items-center gap-1.5 rounded-md border border-border bg-panel px-2 py-1 text-[11px] text-muted xl:flex">
            <Radio className={cn("size-3", live ? "text-success" : "text-warning")} />
            {PLATFORM_LABELS[platform]}
          </span>
        )}

        <Button
          variant="ghost"
          size="sm"
          onClick={onRefresh}
          disabled={busy}
          title={`Refresh windows (${MOD}R)`}
          aria-label="Refresh windows"
          className="h-8 gap-1.5 text-[12px]"
        >
          <RefreshCw className={cn("size-3.5", busy && "animate-spin")} />
          <span className="hidden lg:inline">Refresh windows</span>
        </Button>

        <Button
          variant="secondary"
          size="sm"
          onClick={onOpenStream}
          disabled={!streamEnabled || streamControlsDisabled}
          title="Bring the running Stream output forward for viewing"
          aria-label="Open Stream"
          className="h-8 gap-1.5 text-[12px]"
        >
          <Sparkles className="size-3.5 text-info" />
          <span className="hidden sm:inline">Open Stream</span>
        </Button>

        <label className="inline-flex h-8 items-center gap-2 px-1 text-[12px] text-muted">
          Stream
          <Switch
            checked={streamEnabled}
            onCheckedChange={onToggleStream}
            disabled={streamControlsDisabled}
          />
        </label>

        <Button
          size="sm"
          onClick={onToggleArm}
          title={armed ? "Pause protection and automation; keep saved rules" : "Apply enabled protection rules"}
          aria-pressed={armed}
          disabled={busy}
          className="shield-action h-8 text-[12px]"
        >
          <span className="relative z-[1] flex items-center gap-1.5">
            {armed ? <ShieldCheck className="size-3.5" /> : <Shield className="size-3.5" />}
            {armed ? "Disarm" : "Arm shield"}
          </span>
        </Button>
      </div>
    </header>
  );
}
