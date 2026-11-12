import { CircleDashed, CircleDot, ShieldCheck } from "lucide-react";
import type { Capabilities } from "../types";
import { LEVEL_BADGES, MODES, levelBadge, levelDetail } from "../types";
import { cn } from "../lib/utils";
import { Badge } from "./ui/badge";

function LevelIcon({ level }: { level: "native" | "partial" | "unsupported" }) {
  if (level === "native") return <ShieldCheck className="size-3.5 text-good" />;
  if (level === "partial") return <CircleDot className="size-3.5 text-warn" />;
  return <CircleDashed className="size-3.5 text-ink-faint" />;
}

export function CapabilitiesCard({
  caps,
  matrix,
}: {
  caps: Capabilities | null;
  matrix: Capabilities[];
}) {
  return (
    <section className="rounded-xl border border-line bg-panel">
      <div className="border-b border-line-soft px-4 py-3">
        <h2 className="text-sm font-semibold tracking-tight">Capabilities</h2>
        <p className="mt-0.5 text-xs text-ink-faint">What is actually possible here.</p>
      </div>

      <div className="space-y-1 p-2.5">
        {MODES.map((entry) => {
          const level = caps?.modes[entry.id];
          return (
            <div
              key={entry.id}
              className="rounded-lg px-2.5 py-2 transition-colors hover:bg-row"
            >
              <div className="flex items-center justify-between gap-2">
                <div className="flex items-center gap-2">
                  {level && <LevelIcon level={level.level} />}
                  <span className="text-[13px] font-medium text-ink">{entry.label}</span>
                </div>
                {level && (
                  <Badge
                    variant={
                      level.level === "native"
                        ? "good"
                        : level.level === "partial"
                          ? "sim"
                          : "default"
                    }
                  >
                    {levelBadge(level)}
                  </Badge>
                )}
              </div>
              <p className="mt-1 pl-6 text-xs leading-relaxed text-ink-faint">
                {level ? levelDetail(level) : "Detecting…"}
              </p>
            </div>
          );
        })}
      </div>

      {matrix.length > 0 && (
        <details className="border-t border-line-soft px-4 py-2.5">
          <summary className="cursor-pointer text-xs font-medium text-ink-dim outline-none transition-colors hover:text-ink focus-visible:ring-2 focus-visible:ring-accent/60">
            Full support matrix
          </summary>
          <table className="mt-2 w-full border-collapse text-xs">
            <thead>
              <tr>
                <th className="pb-1.5 text-left font-medium text-ink-faint">Mode</th>
                {matrix.map((entry) => (
                  <th
                    key={entry.platform}
                    className="pb-1.5 text-center font-medium text-ink-faint"
                  >
                    {entry.platform === "mac_os"
                      ? "macOS"
                      : entry.platform === "linux_x11"
                        ? "X11"
                        : entry.platform === "linux_wayland"
                          ? "Wayland"
                          : "Windows"}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {MODES.map((entry) => (
                <tr key={entry.id}>
                  <td className="py-1 pr-2 text-ink-dim">{entry.label}</td>
                  {matrix.map((platformCaps) => {
                    const level = platformCaps.modes[entry.id];
                    return (
                      <td
                        key={platformCaps.platform}
                        className={cn(
                          "py-1 text-center",
                          level.level === "native" && "text-good",
                          level.level === "partial" && "text-warn",
                          level.level === "unsupported" && "text-ink-faint",
                        )}
                        title={levelDetail(level)}
                      >
                        {LEVEL_BADGES[level.level]}
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>
          <p className="mt-2 text-[11px] leading-relaxed text-ink-faint">
            ✓ native · ◐ partial · — unsupported. Exclusion is never claimed on macOS, X11 or
            Wayland.
          </p>
        </details>
      )}
    </section>
  );
}
