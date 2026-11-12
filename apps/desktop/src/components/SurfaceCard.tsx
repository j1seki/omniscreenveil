import { AppWindow, RefreshCw } from "lucide-react";
import type { SurfacePreview } from "../types";
import { cn } from "../lib/utils";
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
  return (
    <section className="rounded-xl border border-line bg-panel">
      <div className="flex items-center justify-between gap-3 border-b border-line-soft px-4 py-3">
        <div>
          <h2 className="text-sm font-semibold tracking-tight">Veil Surface</h2>
          <p className="mt-0.5 text-xs text-ink-faint">
            {preview
              ? `${preview.width}×${preview.height} · ${preview.masked} masked`
              : "Rendering…"}
          </p>
        </div>
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="icon" onClick={onRender} disabled={busy} aria-label="Render">
            <RefreshCw className={cn("size-3.5", busy && "animate-spin")} />
          </Button>
          <Button size="sm" onClick={onOpen} disabled={busy}>
            <AppWindow className="size-3.5" />
            Open
          </Button>
        </div>
      </div>

      <div className="p-3">
        <div className="relative aspect-[16/10] overflow-hidden rounded-lg border border-line-soft bg-void">
          {preview?.png_base64 ? (
            <img
              src={`data:image/png;base64,${preview.png_base64}`}
              alt="Veil Surface preview with protected windows masked"
              className="size-full object-contain"
            />
          ) : (
            <div className="flex size-full items-center justify-center text-xs text-ink-faint">
              No surface rendered yet.
            </div>
          )}
        </div>
        <p className="mt-2.5 px-1 text-xs leading-relaxed text-ink-dim">
          Share this window instead of your whole desktop. Protected windows stay usable locally
          and appear masked to your audience.
        </p>
      </div>
    </section>
  );
}
