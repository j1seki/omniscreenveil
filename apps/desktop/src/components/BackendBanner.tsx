import { AlertTriangle, Info, XCircle } from "lucide-react";
import { cn } from "../lib/utils";

export function BackendBanner({
  tone,
  title,
  children,
}: {
  tone: "info" | "warning" | "error";
  title: string;
  children: React.ReactNode;
}) {
  const Icon = tone === "error" ? XCircle : tone === "warning" ? AlertTriangle : Info;
  return (
    <div
      role={tone === "error" ? "alert" : undefined}
      className={cn(
        "flex items-center gap-2.5 rounded-lg border px-3 py-2 text-[11px] leading-snug",
        tone === "info" && "border-info/20 bg-info-soft text-muted",
        tone === "warning" && "border-warning/25 bg-warning-soft text-muted",
        tone === "error" && "border-danger/30 bg-danger-soft text-muted",
      )}
    >
      <Icon
        className={cn(
          "size-3.5 shrink-0",
          tone === "info" && "text-info",
          tone === "warning" && "text-warning",
          tone === "error" && "text-danger",
        )}
      />
      <span className="min-w-0 flex-1 truncate">
        <strong
          className={cn(
            "font-semibold",
            tone === "info" && "text-info",
            tone === "warning" && "text-warning",
            tone === "error" && "text-danger",
          )}
        >
          {title}
        </strong>{" "}
        {children}
      </span>
    </div>
  );
}
