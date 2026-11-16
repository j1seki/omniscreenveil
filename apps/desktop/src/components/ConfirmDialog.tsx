import { useEffect } from "react";
import { AlertTriangle } from "lucide-react";
import { cn } from "../lib/utils";
import { Button } from "./ui/button";

export function ConfirmDialog({
  open,
  title,
  description,
  confirmLabel = "Confirm",
  danger = false,
  onConfirm,
  onCancel,
}: {
  open: boolean;
  title: string;
  description: string;
  confirmLabel?: string;
  danger?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onCancel]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 px-4 backdrop-blur-sm animate-fade-in"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onCancel();
      }}
    >
      <div
        role="alertdialog"
        aria-modal="true"
        className="w-full max-w-[400px] rounded-xl border border-border-strong bg-panel p-5 shadow-2xl shadow-black/40 animate-scale-in"
      >
        <div className="flex gap-3.5">
          <div
            className={cn(
              "flex size-9 shrink-0 items-center justify-center rounded-lg border",
              danger
                ? "border-danger/30 bg-danger-soft text-danger"
                : "border-accent/30 bg-accent-soft text-accent",
            )}
          >
            <AlertTriangle className="size-4" />
          </div>
          <div className="min-w-0">
            <h2 className="text-[14px] font-semibold text-text">{title}</h2>
            <p className="mt-1.5 text-[12px] leading-relaxed text-muted">{description}</p>
          </div>
        </div>
        <div className="mt-5 flex justify-end gap-2">
          <Button variant="ghost" size="sm" onClick={onCancel} className="text-[12px]">
            Cancel
          </Button>
          <Button
            size="sm"
            onClick={onConfirm}
            className={cn(
              "text-[12px]",
              danger && "bg-danger text-white hover:bg-danger/90",
            )}
          >
            {confirmLabel}
          </Button>
        </div>
      </div>
    </div>
  );
}
