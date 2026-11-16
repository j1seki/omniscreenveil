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
    <section className="rounded-xl border border-line bg-panel/95 shadow-lg">
      <div className="flex items-center gap-2.5 border-b border-line-soft px-3.5 py-2.5">
        <span className="shrink-0 whitespace-nowrap rounded border border-good/40 bg-good/10 px-1.5 py-0.5 font-mono text-[10px] font-bold text-good">
          05 // MATRIX
        </span>
        <div>
          <h2 className="font-mono text-xs font-bold tracking-wide text-ink uppercase">
            OS Capability Bus
          </h2>
          <p className="font-mono text-[10px] text-ink-faint">
            ZERO FAKE PROMISES · VERIFIED PUBLIC APIS ONLY
          </p>
        </div>
      </div>

      <div className="space-y-1.5 p-2.5">
        {MODES.map((entry) => {
          const level = caps?.modes[entry.id];
          return (
            <div
              key={entry.id}
              className="rounded-lg border border-line-soft bg-void/50 px-2.5 py-2 transition-colors hover:border-line"
            >
              <div className="flex items-center justify-between gap-2">
                <div className="flex items-center gap-2">
                  {level && <LevelIcon level={level.level} />}
                  <span className="font-mono text-xs font-bold text-ink">
                    {entry.label}
                  </span>
                  <span className="font-mono text-[9px] text-ink-faint">
                    [{entry.shortLabel}]
                  </span>
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
                    className="font-mono text-[9px] uppercase"
                  >
                    {levelBadge(level)}
                  </Badge>
                )}
              </div>
              <p className="mt-1 pl-5 font-mono text-[10px] leading-relaxed text-ink-dim">
                {level ? levelDetail(level) : "Detecting…"}
              </p>
            </div>
          );
        })}
      </div>

      {matrix.length > 0 && (
        <div className="border-t border-line-soft px-3.5 py-2.5 font-mono">
          <div className="mb-1.5 flex items-center justify-between text-[10px] text-ink-dim">
            <span className="font-bold uppercase">CROSS_OS_SUPPORT_TABLE</span>
            <span className="text-[9px] text-ink-faint">✓ NATIVE · ◐ PARTIAL · — NONE</span>
          </div>
          <table className="w-full border-collapse text-[10px]">
            <thead>
              <tr className="border-b border-line-soft">
                <th className="pb-1.5 text-left font-semibold text-ink-faint">MODE</th>
                {matrix.map((entry) => (
                  <th
                    key={entry.platform}
                    className="pb-1.5 text-center font-semibold text-ink-faint"
                  >
                    {entry.platform === "mac_os"
                      ? "macOS"
                      : entry.platform === "linux_x11"
                        ? "X11"
                        : entry.platform === "linux_wayland"
                          ? "Wayland"
                          : "Win32"}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {MODES.map((entry) => (
                <tr key={entry.id} className="border-b border-line-soft/40 last:border-0">
                  <td className="py-1 pr-2 font-medium text-ink-dim">{entry.label}</td>
                  {matrix.map((platformCaps) => {
                    const level = platformCaps.modes[entry.id];
                    return (
                      <td
                        key={platformCaps.platform}
                        className={cn(
                          "py-1 text-center font-bold",
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
        </div>
      )}
    </section>
  );
}
