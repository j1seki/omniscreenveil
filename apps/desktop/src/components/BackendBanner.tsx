import { AlertTriangle, Info, XCircle } from "lucide-react";

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
    info: "border-line bg-panel text-ink-dim",
    warning: "border-warn/30 bg-warn/10 text-warn",
    error: "border-bad/30 bg-bad/10 text-bad",
  } as const;
  const Icon = tone === "error" ? XCircle : tone === "warning" ? AlertTriangle : Info;
  return (
    <div
      role={tone === "error" ? "alert" : undefined}
      className={`flex items-start gap-2.5 rounded-lg border px-3.5 py-2.5 text-xs leading-relaxed ${styles[tone]}`}
    >
      <Icon className="mt-0.5 size-4 shrink-0" />
      <div>
        <strong className="font-semibold">{title}</strong> {children}
      </div>
    </div>
  );
}
