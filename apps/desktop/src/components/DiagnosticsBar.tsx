import { useCallback, useEffect, useRef, useState } from "react";
import { ChevronDown, ChevronUp, Code2, Copy, RefreshCw, Terminal, Trash2, WrapText } from "lucide-react";
import * as api from "../api";
import type { DiagnosticState, LogEntry } from "../types";
import { cn } from "../lib/utils";
import { Button } from "./ui/button";

const LEVELS = { trace: 0, debug: 1, info: 2, warn: 3, error: 4 };
const TIME = new Intl.DateTimeFormat(undefined, {
  hour: "2-digit", minute: "2-digit", second: "2-digit", fractionalSecondDigits: 3, hourCycle: "h23",
});

function formatLog(entry: LogEntry, timestamps: boolean) {
  const time = timestamps ? `[${TIME.format(entry.timestamp)}] ` : "";
  return `${time}${entry.level.toUpperCase()} [${entry.source}] ${entry.message}`;
}

export function DiagnosticsBar({ notify }: { notify: (message: string, kind?: "success" | "error" | "info") => void }) {
  const [open, setOpen] = useState(false);
  const [level, setLevel] = useState<"trace" | "debug" | "info">("trace");
  const [wrap, setWrap] = useState(false);
  const [timestamps, setTimestamps] = useState(() => {
    try { return localStorage.getItem("veil-log-timestamps") !== "off"; }
    catch { return true; }
  });
  const [state, setState] = useState<DiagnosticState | null>(null);
  const [entries, setEntries] = useState<LogEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const pending = useRef(0);
  const clearing = useRef(false);

  const refresh = useCallback(async () => {
    if (clearing.current) return;
    const request = ++pending.current;
    try {
      const current = await api.diagnosticState().then(
        (value) => ({ value, error: null }),
        (err) => ({ value: null, error: String(err) }),
      );
      const logs = await api.getLogs();
      if (request === pending.current) {
        setState(current.value);
        setEntries(logs);
        setError(current.error);
      }
    } catch (err) {
      if (request === pending.current) setError(String(err));
    }
  }, []);

  useEffect(() => {
    if (!open) return;
    void refresh();
    const timer = window.setInterval(() => { if (!document.hidden) void refresh(); }, 1500);
    return () => { ++pending.current; window.clearInterval(timer); };
  }, [open, refresh]);

  async function clear() {
    if (clearing.current) return;
    clearing.current = true;
    ++pending.current;
    try {
      await api.clearLogs();
      setEntries([]);
      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      clearing.current = false;
    }
  }

  const protection = state?.protection;
  const lastFrame = state?.last_frames.surface ?? state?.last_frames.main;
  const visible = entries.filter((entry) => LEVELS[entry.level] >= LEVELS[level]).reverse();
  async function copy() {
    try {
      await api.copyLogs(visible.map((entry) => formatLog(entry, timestamps)).join("\n"));
      notify("Logs copied", "info");
    } catch (err) {
      setError(String(err));
    }
  }

  return (
    <footer className="shrink-0 border-t border-border bg-app">
      <div className="flex h-9 items-center justify-between px-6">
        <Button variant="ghost" size="sm" className="h-7 px-2 text-[11px]" aria-expanded={open} aria-controls="desktop-logs" onClick={() => setOpen((value) => !value)}>
          <Terminal className="size-3.5" /> Logs {open ? <ChevronDown className="size-3" /> : <ChevronUp className="size-3" />}
        </Button>
        <Button variant="ghost" size="sm" className="h-7 px-2 text-[11px]" disabled={!api.IN_TAURI} onClick={() => void api.openDevTools().catch((err) => notify(String(err), "error"))}>
          <Code2 className="size-3.5" /> Open DevTools
        </Button>
      </div>
      {open && (
        <section id="desktop-logs" aria-label="Desktop logs" className="border-t border-border bg-panel">
          <div className="flex flex-wrap items-center gap-1.5 border-b border-border px-6 py-2">
            <label className="mr-2 flex items-center gap-2 text-[11px] text-muted">
              Level
              <select className="rounded-md border border-border bg-app px-2 py-1 text-text" value={level} onChange={(e) => setLevel(e.target.value as typeof level)}>
                <option value="trace">Full</option>
                <option value="debug">Debug</option>
                <option value="info">Info</option>
              </select>
            </label>
            <Button variant="ghost" size="sm" aria-pressed={wrap} className={cn("h-7 px-2 text-[11px]", wrap && "bg-accent-soft text-accent")} onClick={() => setWrap((value) => !value)}><WrapText className="size-3.5" /> Wrap</Button>
            <label className="flex items-center gap-1.5 px-2 text-[11px] text-muted">
              <input type="checkbox" className="accent-accent" checked={timestamps} onChange={(event) => {
                const next = event.target.checked;
                setTimestamps(next);
                try { localStorage.setItem("veil-log-timestamps", next ? "on" : "off"); } catch {}
              }} />
              Timestamps
            </label>
            <Button variant="ghost" size="sm" className="h-7 px-2 text-[11px]" disabled={!visible.length} onClick={() => void copy()}><Copy className="size-3.5" /> Copy</Button>
            <Button variant="ghost" size="sm" className="h-7 px-2 text-[11px]" disabled={!entries.length} onClick={() => void clear()}><Trash2 className="size-3.5" /> Clear logs</Button>
            <Button variant="ghost" size="sm" className="h-7 px-2 text-[11px]" onClick={() => void refresh()}><RefreshCw className="size-3.5" /> Refresh</Button>
          </div>
          {protection && (
            <div className="flex flex-wrap items-center gap-x-3 gap-y-1 border-b border-border px-6 py-2 text-[11px] text-muted">
              <strong className={protection.failures.length || protection.profile_error ? "text-warning" : "text-text"}>
                {protection.profile_error ? "Profile unavailable" : protection.failures.length && !protection.armed ? "Disarm incomplete" : protection.armed ? "Armed" : "Disarmed"}
                {!protection.live && " (simulation)"}
              </strong>
              <span>Automation {protection.automation_paused ? "paused" : protection.policy.replaceAll("_", " ")}</span>
              <span>{protection.excluded} excluded · {protection.surface_rules_active} Surface rules active · {protection.hidden} hidden</span>
              {(protection.failures.length > 0 || protection.awaiting_apply > 0) && <span className="text-warning">{protection.failures.length} failed · {protection.awaiting_apply} pending</span>}
              {lastFrame && <span title={lastFrame.path}>Last capture {lastFrame.width}×{lastFrame.height} · {lastFrame.masked} masked · {lastFrame.excluded} excluded</span>}
            </div>
          )}
          {error && <p role="alert" className="px-6 py-2 text-[11px] text-danger">{error}</p>}
          <div className="h-40 max-h-[24vh] overflow-auto px-6 py-2 font-mono text-[11px] leading-relaxed">
            {visible.length ? (
              <ul className={wrap ? "whitespace-pre-wrap break-words" : "whitespace-pre"}>
                {visible.map((entry, index) => (
                  <li key={`${entry.timestamp}-${entries.length - index}`} className={entry.level === "error" ? "text-danger" : entry.level === "warn" ? "text-warning" : "text-muted"}>{formatLog(entry, timestamps)}</li>
                ))}
              </ul>
            ) : <p className="text-muted">No logs to display.</p>}
          </div>
        </section>
      )}
    </footer>
  );
}
