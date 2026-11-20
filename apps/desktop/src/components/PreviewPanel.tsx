import type { ReactNode } from "react";
import { RefreshCw } from "lucide-react";
import type { CaptureFrame } from "../types";
import { CaptureImage } from "./CaptureImage";
import { Button } from "./ui/button";

export interface PreviewPanelProps {
  frame: CaptureFrame | null;
  message: string;
  capture: () => void;
  manual: boolean;
  ready: boolean;
  actions?: ReactNode;
}

export function PreviewPanel({ frame, message, capture, manual, ready, actions }: PreviewPanelProps) {
  return (
    <section className="overflow-hidden rounded-xl border border-border bg-panel">
      <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border px-4 py-2.5">
        <div>
          <h2 className="text-[13px] font-medium">Desktop preview</h2>
          {frame && (
            <p className="mt-0.5 text-[11px] text-muted">
              {frame.width}×{frame.height} · {frame.armed || frame.masked + frame.excluded > 0
                ? `${frame.masked} masked · ${frame.excluded} excluded`
                : "Protection disarmed"}
            </p>
          )}
        </div>
        <div className="flex flex-wrap items-center gap-2">
          {actions}
          <Button variant="secondary" size="sm" onClick={capture} disabled={!ready}>
            <RefreshCw className="size-3.5" /> {manual ? "Capture frame" : "Refresh preview"}
          </Button>
        </div>
      </div>
      <div data-capture-preview className="relative grid aspect-video place-items-center bg-app">
        {frame ? (
          <CaptureImage image={frame.image} alt="Filtered desktop preview" className="absolute inset-0 size-full object-contain" />
        ) : (
          <p role="status" className="px-4 text-center text-[12px] text-muted">{message}</p>
        )}
      </div>
    </section>
  );
}
