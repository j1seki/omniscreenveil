import { CheckCircle2, HelpCircle, ScanSearch, XCircle } from "lucide-react";
import type { VerificationRun } from "../types";
import { cn } from "../lib/utils";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";

function Outcome({ outcome }: { outcome: VerificationRun["outcome"] }) {
  if (outcome === "protected") {
    return (
      <span className="inline-flex items-center gap-1.5 text-good">
        <CheckCircle2 className="size-4" />
        Protected
      </span>
    );
  }
  if (outcome === "visible") {
    return (
      <span className="inline-flex items-center gap-1.5 text-bad">
        <XCircle className="size-4" />
        Visible
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1.5 text-warn">
      <HelpCircle className="size-4" />
      Inconclusive
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
    <section className="rounded-xl border border-line bg-panel">
      <div className="flex items-center justify-between gap-3 border-b border-line-soft px-4 py-3">
        <div>
          <h2 className="text-sm font-semibold tracking-tight">Verification</h2>
          <p className="mt-0.5 text-xs text-ink-faint">Pixels, not return codes.</p>
        </div>
        <Button size="sm" onClick={onRun} disabled={busy}>
          <ScanSearch className={cn("size-3.5", busy && "animate-pulse")} />
          {busy ? "Capturing…" : "Run"}
        </Button>
      </div>

      <div className="p-4">
        {verification ? (
          <div className="space-y-3">
            <div className="flex items-center justify-between gap-2">
              <Outcome outcome={verification.outcome} />
              <Badge variant="default" className="font-mono">
                {verification.record.path}
              </Badge>
            </div>

            <div className="grid grid-cols-2 gap-2">
              <div
                className={cn(
                  "rounded-lg border px-3 py-2",
                  verification.baseline.found
                    ? "border-warn/30 bg-warn/10"
                    : "border-line bg-row",
                )}
              >
                <p className="text-[11px] uppercase tracking-wide text-ink-faint">Baseline</p>
                <p className="mt-0.5 text-xs font-medium text-ink">
                  {verification.baseline.found ? "canary visible" : "not seen"}
                </p>
              </div>
              <div
                className={cn(
                  "rounded-lg border px-3 py-2",
                  verification.after.found
                    ? "border-bad/30 bg-bad/10"
                    : "border-good/30 bg-good/10",
                )}
              >
                <p className="text-[11px] uppercase tracking-wide text-ink-faint">After arming</p>
                <p className="mt-0.5 text-xs font-medium text-ink">
                  {verification.after.found ? "canary visible" : "canary absent"}
                </p>
              </div>
            </div>

            <p className="text-xs leading-relaxed text-ink-dim">{verification.record.detail}</p>

            <dl className="grid grid-cols-2 gap-x-3 gap-y-1.5 border-t border-line-soft pt-3 text-xs">
              <div className="flex justify-between gap-2">
                <dt className="text-ink-faint">Checked</dt>
                <dd className="text-ink-dim">
                  {new Date(verification.record.checked_at_unix * 1000).toLocaleString()}
                </dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt className="text-ink-faint">Build</dt>
                <dd className="font-mono text-ink-dim">{verification.record.os_build}</dd>
              </div>
            </dl>
          </div>
        ) : (
          <p className="text-xs leading-relaxed text-ink-dim">
            Runs the headless harness: composite an 8×8 canary, capture, arm, capture again. A
            missing baseline is reported as inconclusive, never as protected.
          </p>
        )}
      </div>
    </section>
  );
}
