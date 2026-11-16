import { useState } from "react";
import { AppWindow, Crosshair, RefreshCw } from "lucide-react";
import type { SurfacePreview } from "../types";
import { cn } from "../lib/utils";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";

export function SurfaceCard({
  preview,
  busy,
  onRender,
  onOpen,
}: {
  preview: SurfacePreview | null;
  busy: boolean;
  onRender: () => void;
  onOpen: () => void;
}) {
  const [hudOverlay, setHudOverlay] = useState(true);

  return (
    <section className="rounded-xl border border-line bg-panel/95 shadow-lg">
      <div className="flex items-center justify-between gap-2 border-b border-line-soft px-3.5 py-2.5">
        <div className="flex min-w-0 items-center gap-2">
          <span className="shrink-0 whitespace-nowrap rounded border border-cyan/40 bg-cyan/10 px-1.5 py-0.5 font-mono text-[10px] font-bold text-cyan">
            02 // SURFACE
          </span>
          <div className="min-w-0">
            <h2 className="truncate font-mono text-xs font-bold tracking-wide text-ink uppercase">
              Veil Compositor
            </h2>
            <p className="truncate font-mono text-[10px] text-ink-faint">
              {preview
                ? `${preview.width}×${preview.height} · ${preview.masked} MASKED`
                : "INITIALIZING…"}
            </p>
          </div>
        </div>

        <div className="flex shrink-0 items-center gap-1">
          <Button
            variant={hudOverlay ? "secondary" : "ghost"}
            size="sm"
            onClick={() => setHudOverlay((v) => !v)}
            className="h-7 px-2 font-mono text-[10px]"
          >
            <Crosshair className="size-3 text-cyan" />
            HUD
          </Button>
          <Button
            variant="ghost"
            size="icon"
            onClick={onRender}
            disabled={busy}
            aria-label="Render"
            className="size-7"
          >
            <RefreshCw className={cn("size-3.5", busy && "animate-spin")} />
          </Button>
          <Button
            size="sm"
            onClick={onOpen}
            disabled={busy}
            className="h-7 gap-1 px-2.5 font-mono text-[11px]"
          >
            <AppWindow className="size-3.5" />
            Open
          </Button>
        </div>
      </div>

      <div className="p-3">
        <div className="relative aspect-[16/10] overflow-hidden rounded-lg border border-line bg-void">
          {preview?.png_base64 ? (
            <img
              src={`data:image/png;base64,${preview.png_base64}`}
              alt="Veil Surface preview with protected windows masked"
              className="size-full object-contain"
            />
          ) : (
            <div className="flex size-full flex-col items-center justify-center gap-1 font-mono text-xs text-ink-faint">
              <span>[ AWAITING_COMPOSITOR_FRAME ]</span>
            </div>
          )}

          {hudOverlay && (
            <div
              aria-hidden="true"
              className="pointer-events-none absolute inset-0 flex flex-col justify-between p-2 font-mono text-[9px] text-accent/80"
            >
              <div className="flex items-center justify-between">
                <span className="rounded bg-void/80 px-1.5 py-0.5 border border-line-soft">
                  ┌ COMPOSITE_STREAM // {preview ? `${preview.width}×${preview.height}` : "IDLE"}
                </span>
                <Badge
                  variant={preview && preview.masked > 0 ? "accent" : "default"}
                  className="bg-void/85 font-mono text-[9px]"
                >
                  {preview ? `${preview.masked} REGIONS VEILED` : "0 VEILED"}
                </Badge>
              </div>
              <div className="flex items-center justify-between text-ink-faint">
                <span className="rounded bg-void/80 px-1.5 py-0.5">
                  └ {preview?.source ?? "no source"}
                </span>
                <span className="rounded bg-void/80 px-1.5 py-0.5 text-cyan">
                  PREVIEW ONLY ┘
                </span>
              </div>
            </div>
          )}
        </div>

        <p className="mt-2 px-0.5 font-mono text-[10px] leading-relaxed text-ink-dim">
          Share the dedicated <strong className="text-ink">OmniScreenVeil Surface</strong> window in
          OBS, Discord, Zoom, or browsers. Protected windows stay usable locally while veiled with
          the checker pattern for your audience.
        </p>
      </div>
    </section>
  );
}
