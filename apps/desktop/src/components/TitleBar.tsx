import { ChevronRight, Command, Radio, RefreshCw, Shield, Sparkles } from "lucide-react";
import type { BackendInfo, Platform } from "../types";
import { PLATFORM_LABELS } from "../types";
import { cn } from "../lib/utils";
import type { Page } from "./Sidebar";
import { Button } from "./ui/button";

export const MOD = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform) ? "⌘" : "Ctrl+";

const TITLES: Record<Page, { title: string; subtitle: string }> = {
  overview: { title: "Overview", subtitle: "Your capture privacy at a glance" },
  windows: { title: "Windows", subtitle: "Choose which windows are protected and how" },
  surface: { title: "Veil Surface", subtitle: "The shareable, filtered view of your desktop" },
  audit: { title: "Canary audit", subtitle: "Synthetic pixel self-test of the veil compositor" },
  rules: { title: "Rules & automation", subtitle: "Persistent protection policies and auto-arm" },
  capabilities: { title: "Capabilities", subtitle: "What each operating system can actually do" },
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
  onOpenSurface,
}: {
  page: Page;
  backend: BackendInfo | null;
  platform: Platform | null;
  armed: boolean;
  busy: boolean;
  onOpenPalette: () => void;
  onRefresh: () => void;
  onToggleArm: () => void;
  onOpenSurface: () => void;
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
          size="icon"
          onClick={onRefresh}
          disabled={busy}
          title="Refresh (⌘R)"
          className="size-8"
        >
          <RefreshCw className={cn("size-3.5", busy && "animate-spin")} />
        </Button>

        <Button
          variant="secondary"
          size="sm"
          onClick={onOpenSurface}
          className="h-8 gap-1.5 text-[12px]"
        >
          <Sparkles className="size-3.5 text-info" />
          <span className="hidden sm:inline">Open Surface</span>
        </Button>

        <Button
          size="sm"
          onClick={onToggleArm}
          disabled={busy}
          className={cn(
            "h-8 gap-1.5 text-[12px]",
            armed && "border border-border-strong bg-elevated text-text shadow-none hover:bg-hover",
          )}
        >
          <Shield className={cn("size-3.5", !armed && "text-white")} />
          {armed ? "Disarm" : "Arm shield"}
        </Button>
      </div>
    </header>
  );
}
