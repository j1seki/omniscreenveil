import { CheckCircle2, Info, X, XCircle } from "lucide-react";
import { cn } from "../lib/utils";

export interface Toast {
  id: number;
  message: string;
  kind: "success" | "info" | "error";
}

export function Toaster({
  toasts,
  onDismiss,
}: {
  toasts: Toast[];
  onDismiss: (id: number) => void;
}) {
  if (toasts.length === 0) return null;
  return (
    <div className="pointer-events-none fixed right-5 bottom-5 z-[60] flex w-[320px] flex-col gap-2">
      {toasts.map((toast) => {
        const Icon =
          toast.kind === "success" ? CheckCircle2 : toast.kind === "error" ? XCircle : Info;
        return (
          <div
            key={toast.id}
            className="pointer-events-auto flex items-start gap-2.5 rounded-lg border border-border-strong bg-elevated p-3 shadow-xl shadow-black/20 animate-toast-in"
          >
            <Icon
              className={cn(
                "mt-0.5 size-4 shrink-0",
                toast.kind === "success" && "text-success",
                toast.kind === "error" && "text-danger",
                toast.kind === "info" && "text-info",
              )}
            />
            <p className="flex-1 text-[12px] leading-snug text-text">{toast.message}</p>
            <button
              type="button"
              onClick={() => onDismiss(toast.id)}
              className="text-faint transition-colors hover:text-text"
            >
              <X className="size-3.5" />
            </button>
          </div>
        );
      })}
    </div>
  );
}
