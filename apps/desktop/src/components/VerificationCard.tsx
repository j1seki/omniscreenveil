import { CheckCircle2, HelpCircle, ScanSearch, XCircle } from "lucide-react";
import type { VerificationRun } from "../types";
import { cn } from "../lib/utils";
import { CanaryMatrixVisualizer } from "./AsciiMotion";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";

function Outcome({ outcome }: { outcome: VerificationRun["outcome"] }) {
  if (outcome === "protected") {
    return (
      <span className="inline-flex items-center gap-1.5 font-mono text-xs font-bold text-good uppercase">
        <CheckCircle2 className="size-4" />
        SELF-TEST PASSED
      </span>
    );
  }
  if (outcome === "visible") {
    return (
      <span className="inline-flex items-center gap-1.5 font-mono text-xs font-bold text-bad uppercase">
        <XCircle className="size-4" />
        LEAK VISIBLE
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1.5 font-mono text-xs font-bold text-warn uppercase">
      <HelpCircle className="size-4" />
      INCONCLUSIVE
    </span>
  );
}

export function VerificationCard({
  verification,
  busy,
  onRun,
}: {
  verification: VerificationRun | null;
  busy: boolean;
  onRun: () => void;
}) {
  return (
    <section className="rounded-xl border border-line bg-panel/95 shadow-lg">
      <div className="flex items-center justify-between gap-2 border-b border-line-soft px-3.5 py-2.5">
        <div className="flex min-w-0 items-center gap-2.5">
          <span className="shrink-0 whitespace-nowrap rounded border border-violet/40 bg-violet/10 px-1.5 py-0.5 font-mono text-[10px] font-bold text-violet">
            03 // CANARY
          </span>
          <div className="min-w-0">
            <h2 className="truncate font-mono text-xs font-bold tracking-wide text-ink uppercase">
              8×8 Pixel Verification
            </h2>
            <p className="truncate font-mono text-[10px] text-ink-faint">
              SYNTHETIC SELF-TEST · NOT LIVE CAPTURE
            </p>
          </div>
        </div>

        <Button
          size="sm"
          onClick={onRun}
          disabled={busy}
          className="h-7 gap-1.5 font-mono text-[11px]"
        >
          <ScanSearch className={cn("size-3.5", busy && "animate-pulse")} />
          {busy ? "Auditing…" : "Run Audit"}
        </Button>
      </div>

      <div className="space-y-2.5 p-3">
        <CanaryMatrixVisualizer verification={verification} busy={busy} />

        {verification ? (
          <div className="space-y-2 rounded-lg border border-line-soft bg-void/60 p-2.5">
            <div className="flex items-center justify-between gap-2">
              <Outcome outcome={verification.outcome} />
              <Badge variant="default" className="font-mono text-[10px] uppercase">
                {verification.record.path}
              </Badge>
            </div>

            <div className="grid grid-cols-2 gap-2 font-mono">
              <div
                className={cn(
                  "rounded border px-2.5 py-1.5",
                  verification.baseline.found
                    ? "border-warn/35 bg-warn/10"
                    : "border-line bg-row",
                )}
              >
                <p className="text-[9px] uppercase tracking-wider text-ink-faint">
                  Baseline Capture
                </p>
                <p className="mt-0.5 text-[11px] font-bold text-ink">
                  {verification.baseline.found ? `canary visible (${verification.baseline.matched}/64)` : "not seen"}
                </p>
              </div>
              <div
                className={cn(
                  "rounded border px-2.5 py-1.5",
                  verification.after.found
                    ? "border-bad/35 bg-bad/10"
                    : "border-good/35 bg-good/10",
                )}
              >
                <p className="text-[9px] uppercase tracking-wider text-ink-faint">
                  After Arming
                </p>
                <p className="mt-0.5 text-[11px] font-bold text-ink">
                  {verification.after.found ? "canary visible" : `canary absent (${verification.after.matched}/64)`}
                </p>
              </div>
            </div>

            <p className="font-mono text-[10px] leading-relaxed text-ink-dim">
              {verification.record.detail}
            </p>

            <dl className="grid grid-cols-2 gap-x-3 border-t border-line-soft pt-2 font-mono text-[10px]">
              <div className="flex justify-between gap-1">
                <dt className="text-ink-faint">CHECKED</dt>
                <dd className="truncate text-ink-dim">
                  {new Date(verification.record.checked_at_unix * 1000).toLocaleTimeString()}
                </dd>
              </div>
              <div className="flex justify-between gap-1">
                <dt className="text-ink-faint">BUILD</dt>
                <dd className="truncate text-cyan">{verification.record.os_build}</dd>
              </div>
            </dl>
          </div>
        ) : (
          <p className="font-mono text-[10px] leading-relaxed text-ink-dim">
            Tests masking on synthetic pixels only. It does not verify your windows, OS capture,
            or what your audience can see. Test your actual sharing application before sharing.
          </p>
        )}
      </div>
    </section>
  );
}
