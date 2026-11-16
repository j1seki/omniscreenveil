import { AppWindow, Cpu, Radio, Shield, ShieldAlert, ShieldCheck, Sparkles } from "lucide-react";
import type { BackendInfo, Platform } from "../types";
import { PLATFORM_DRIVERS, PLATFORM_LABELS } from "../types";
import { cn } from "../lib/utils";
import { AsciiWaveBanner } from "./AsciiMotion";
import { ShieldMark } from "./icons";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";

export function TopBar({
  backend,
  platform,
  armed = false,
  protectedCount = 0,
  totalWindows = 0,
  captureActive = false,
  busy = false,
  onToggleArm,
  onOpenSurface,
}: {
  backend: BackendInfo | null;
  platform: Platform | null;
  armed?: boolean;
  protectedCount?: number;
  totalWindows?: number;
  captureActive?: boolean;
  busy?: boolean;
  onToggleArm?: () => void;
  onOpenSurface?: () => void;
}) {
  return (
    <header className="flex items-center justify-between gap-3 border-b border-line bg-panel/90 px-4 py-2.5 backdrop-blur-md">
      {/* Brand + Rust Engine Identity */}
      <div className="flex items-center gap-3">
        <div
          className={cn(
            "relative flex size-9 items-center justify-center rounded-lg border transition-all",
            armed
              ? "border-accent bg-accent/15 shadow-[0_0_18px_rgba(0,245,212,0.28)]"
              : "border-line bg-panel-raised",
          )}
        >
          <ShieldMark size={20} className={armed ? "text-accent" : "text-cyan"} />
          <span
            className={cn(
              "absolute -top-0.5 -right-0.5 size-2 rounded-full ring-2 ring-panel",
              armed ? "bg-accent animate-ping" : "bg-good",
            )}
          />
          <span
            className={cn(
              "absolute -top-0.5 -right-0.5 size-2 rounded-full ring-2 ring-panel",
              armed ? "bg-accent" : "bg-good",
            )}
          />
        </div>

        <div>
          <div className="flex items-center gap-2">
            <h1 className="font-mono text-sm font-bold tracking-tight text-ink">
              OMNISCREENVEIL
            </h1>
            <span className="rounded border border-violet/40 bg-violet/15 px-1.5 py-0.5 font-mono text-[9px] font-bold tracking-wider text-violet uppercase">
              RUST // CORE v0.1
            </span>
          </div>
          <p className="mt-0.5 font-mono text-[10px] text-ink-faint">
            {platform ? PLATFORM_DRIVERS[platform] : "SELECTIVE_CAPTURE_PRIVACY_ENGINE"}
          </p>
        </div>
      </div>

      {/* Live ASCII Motion & Telemetry Wave */}
      <AsciiWaveBanner
        armed={armed}
        protectedCount={protectedCount}
        totalWindows={totalWindows}
        captureActive={captureActive}
      />

      {/* Right Status & Reactor Controls */}
      <div className="flex items-center gap-2">
        {captureActive && (
          <Badge
            variant="sim"
            className="hidden md:inline-flex gap-1.5 border-warn/50 bg-warn/15 font-mono text-[10px] text-warn"
          >
            <ShieldAlert className="size-3 animate-pulse" />
            CAPTURE_APP_ACTIVE
          </Badge>
        )}

        {platform && (
          <Badge variant="default" className="hidden sm:inline-flex font-mono text-[10px]">
            <Cpu className="size-3 text-cyan" />
            {PLATFORM_LABELS[platform]}
          </Badge>
        )}

        {backend && (
          <Badge
            variant={backend.live ? "live" : "sim"}
            className="font-mono text-[10px]"
          >
            <Radio className="size-3" />
            {backend.live ? `LIVE: ${backend.name}` : "SIM_PREVIEW"}
          </Badge>
        )}

        {onOpenSurface && (
          <Button
            variant="secondary"
            size="sm"
            onClick={onOpenSurface}
            disabled={busy}
            className="h-8 gap-1.5 border-line font-mono text-[11px]"
          >
            <AppWindow className="size-3.5 text-cyan" />
            <span className="hidden sm:inline">Surface</span>
          </Button>
        )}

        {onToggleArm && (
          <button
            type="button"
            onClick={onToggleArm}
            disabled={busy}
            className={cn(
              "inline-flex h-8 items-center gap-1.5 rounded-lg border px-3 font-mono text-[11px] font-bold tracking-wide uppercase transition-all cursor-pointer disabled:opacity-50",
              armed
                ? "border-accent bg-accent text-void shadow-[0_0_20px_rgba(0,245,212,0.35)] hover:bg-accent/90"
                : "border-accent/50 bg-accent/15 text-accent hover:bg-accent/25 hover:border-accent",
            )}
          >
            {armed ? (
              <>
                <ShieldCheck className="size-3.5" />
                ARMED · DISARM
              </>
            ) : (
              <>
                <Shield className="size-3.5" />
                <Sparkles className="size-3" />
                ARM SHIELD
              </>
            )}
          </button>
        )}
      </div>
    </header>
  );
}
