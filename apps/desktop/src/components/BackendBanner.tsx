import { AlertTriangle, Terminal, XCircle } from "lucide-react";

export function BackendBanner({
  tone,
  title,
  children,
}: {
  tone: "info" | "warning" | "error";
  title: string;
  children: React.ReactNode;
}) {
  const styles = {
    info: "border-cyan/30 bg-cyan/[0.06] text-ink-dim",
    warning: "border-warn/40 bg-warn/[0.08] text-warn",
    error: "border-bad/50 bg-bad/[0.12] text-bad",
  } as const;
  const Icon = tone === "error" ? XCircle : tone === "warning" ? AlertTriangle : Terminal;
  return (
    <div
      role={tone === "error" ? "alert" : undefined}
      className={`flex items-center gap-2.5 rounded-lg border px-3 py-1.5 font-mono text-[11px] leading-snug ${styles[tone]}`}
    >
      <Icon className="size-3.5 shrink-0 text-accent" />
      <div className="min-w-0 flex-1 truncate">
        <strong className="font-bold text-ink">{title}</strong>{" "}
        <span className="opacity-90">{children}</span>
      </div>
    </div>
  );
}
