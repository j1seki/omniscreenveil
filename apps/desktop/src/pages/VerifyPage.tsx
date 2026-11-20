import { useState } from "react";
import type { ReactNode } from "react";
import { CheckCircle2, ChevronRight, Play, TriangleAlert, XCircle } from "lucide-react";
import * as api from "../api";
import type { VerificationCheck } from "../types";
import { Badge } from "../components/ui/badge";
import { Button } from "../components/ui/button";
import { AuditPage } from "./InspectorPages";

const MANUAL = [
  { id: "discord", label: "Third-party Exclusion in Discord or OBS", instruction: "Use a harmless app as the target. Confirm it is visible to you and absent in an actual screen share while Exclusion is active. Disarm and confirm it returns in the receiver view." },
  { id: "surface-share", label: "Surface mode and the Stream share window", instruction: "Apply Surface mode to a sample app. Share OmniScreenVeil Stream and confirm the sample is masked there. Disarm and verify the sample and dashboard controls reappear." },
  { id: "playback", label: "Preview playback and Stream quality", instruction: "Set Preview playback to Manual and check that Overview shows the same frame. Arm an enabled Surface rule and verify Stream starts without opening the viewer. Disarm and check it stops. Turn Stream on manually and verify the toast reports the protection state and enabled Surface rules. Change target FPS and confirm Preview playback stays the same." },
  { id: "auto-arm", label: "Automation, tray, and hotkey", instruction: "Select Capture app and check that protection follows an app such as OBS opening or closing. Disarm with the shortcut or tray, then select Capture app again while OBS is open and confirm it re-arms." },
  { id: "diagnostics", label: "Live logs and DevTools", instruction: "Arm and disarm a sample rule. Compare the log panel with Windows and Stream. Switch levels and timestamps, copy and paste the visible log, refresh, clear it, and open DevTools." },
  { id: "ui", label: "Rules, app icons, and window tools", instruction: "Select a running app with an icon. Add, disable, and delete its rule. Filter the Windows list, refresh it, and check the app in the Spatial Map." },
  { id: "settings", label: "Launch at login, minimize, quit, and appearance", instruction: "Confirm the settings you change are still selected after reopening the app. Check that minimize keeps the session active and quit releases protections." },
  { id: "install", label: "Signed update installation and restart", instruction: "Use a test install with an update available. Check the chosen channel, install the update, restart, and confirm the version and saved profile." },
];

const statusTheme = {
  passed: { icon: CheckCircle2, text: "Passed", variant: "live" },
  failed: { icon: XCircle, text: "Failed", variant: "sim" },
  blocked: { icon: TriangleAlert, text: "Blocked", variant: "default" },
} as const;

function readResults<T>(key: string, fallback: T): T {
  try {
    const value = sessionStorage.getItem(key);
    return value ? JSON.parse(value) as T : fallback;
  } catch { return fallback; }
}

function persistResults<T>(key: string, value: T) {
  try { sessionStorage.setItem(key, JSON.stringify(value)); } catch {}
}

function SuiteSection({ title, badges, children }: { title: string; badges?: ReactNode; children: ReactNode }) {
  return (
    <details className="suite-section rounded-xl border border-border bg-panel">
      <summary className="flex cursor-pointer list-none items-center justify-between gap-3 px-4 py-3 text-sm font-semibold [&::-webkit-details-marker]:hidden">
        <span className="flex items-center gap-2"><ChevronRight className="suite-chevron size-4 shrink-0 text-muted" />{title}</span>
        {badges && <span className="flex flex-wrap gap-1.5">{badges}</span>}
      </summary>
      <div className="border-t border-border p-4">{children}</div>
    </details>
  );
}

export function VerifyPage({ armed, live }: { armed: boolean; live: boolean }) {
  const [checks, setChecks] = useState(() => readResults<VerificationCheck[]>("veil-verify-automated", []));
  const [manual, setManual] = useState(() => readResults<Record<string, "passed" | "failed">>("veil-verify-manual", {}));
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function run() {
    setRunning(true);
    setError(null);
    try {
      const results = await api.runVerification();
      setChecks(results);
      persistResults("veil-verify-automated", results);
    } catch (err) {
      setError(String(err));
    } finally {
      setRunning(false);
    }
  }

  function mark(id: string, outcome: "passed" | "failed") {
    setManual((old) => {
      const next = { ...old };
      if (next[id] === outcome) delete next[id];
      else next[id] = outcome;
      persistResults("veil-verify-manual", next);
      return next;
    });
  }

  const passed = checks.filter((check) => check.status === "passed").length;
  const failed = checks.filter((check) => check.status === "failed").length;
  const blocked = checks.length - passed - failed;
  const groups = [...new Set(checks.map((check) => check.group))];

  return (
    <div className="animate-page-in space-y-4">
      <section className="rounded-xl border border-border bg-panel">
        <div className="flex flex-wrap items-center justify-between gap-3 px-4 py-4">
          <p className="text-[12px] text-muted">On Windows, a temporary test window appears briefly. Stop any active share before running the suite.</p>
          <Button onClick={() => void run()} disabled={running || !api.IN_TAURI}><Play className="size-3.5" /> {running ? "Running checks…" : "Run test suite"}</Button>
        </div>
        {!api.IN_TAURI && <p className="px-4 pb-4 text-xs text-muted">Open the desktop app to run system checks.</p>}
        {error && <p role="alert" className="px-4 pb-4 text-sm text-danger">{error}</p>}
      </section>

      <SuiteSection title="Automated checks" badges={checks.length > 0 && <>
        <Badge variant="live">{passed} passed</Badge>
        <Badge variant={failed ? "sim" : "default"}>{failed} failed</Badge>
        <Badge>{blocked} blocked</Badge>
      </>}>
        {groups.length === 0 ? <p className="text-[12px] text-muted">Run the suite to see results.</p> : (
          <div className="space-y-2">
            {groups.map((group) => (
              <SuiteSection key={group} title={group}>
                <ul className="space-y-2">
                  {checks.filter((check) => check.group === group).map((check) => {
                    const state = statusTheme[check.status];
                    return (
                      <li key={check.id} className="flex items-start justify-between gap-4 rounded-lg border border-border bg-app px-3 py-2">
                        <div>
                          <p className="text-[12px] font-medium">{check.label}</p>
                          <p className="mt-0.5 text-[11px] text-muted">{check.evidence}</p>
                        </div>
                        <Badge variant={state.variant}><state.icon className="size-3" /> {state.text}</Badge>
                      </li>
                    );
                  })}
                </ul>
              </SuiteSection>
            ))}
          </div>
        )}
      </SuiteSection>

      <SuiteSection title="Guided checks">
        <ul className="divide-y divide-border">
          {MANUAL.map((check) => (
            <li key={check.id} className="flex flex-wrap items-start justify-between gap-2 py-3">
              <div className="min-w-56 flex-1">
                <p className="text-[12px] font-medium">{check.label}</p>
                <p className="mt-0.5 text-[11px] text-muted">{check.instruction}</p>
              </div>
              <div className="flex gap-1">
                <Button size="sm" variant={manual[check.id] === "passed" ? "secondary" : "ghost"} onClick={() => mark(check.id, "passed")} aria-pressed={manual[check.id] === "passed"}>Pass</Button>
                <Button size="sm" variant={manual[check.id] === "failed" ? "secondary" : "ghost"} onClick={() => mark(check.id, "failed")} aria-pressed={manual[check.id] === "failed"}>Fail</Button>
              </div>
            </li>
          ))}
        </ul>
      </SuiteSection>

      <SuiteSection title="Manual capture comparison">
        <AuditPage armed={armed} live={live} />
      </SuiteSection>
    </div>
  );
}
