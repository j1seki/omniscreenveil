import { useEffect, useState } from "react";
import * as api from "../api";
import {
  AppWindow,
  ExternalLink,
  RefreshCw,
  ShieldCheck,
  Trash2,
} from "lucide-react";
import type {
  AutoArm,
  AppKey,
  Capabilities,
  Display,
  DesktopPreferences,
  Profile,
  ProtectionMode,
  SurfacePreview,
  WindowInfo,
} from "../types";
import { LEVEL_BADGES, MODES, PLATFORM_LABELS, levelDetail, modeLabel } from "../types";
import { cn } from "../lib/utils";
import { DisplayPicker } from "../components/DisplayPicker";
import { Badge } from "../components/ui/badge";
import { Button } from "../components/ui/button";
import { Switch } from "../components/ui/switch";

/* ------------------------------ Surface ---------------------------------- */
export function SurfacePage({
  preview,
  error,
  busy,
  onRender,
  onDisplayChange,
  onOpen,
  revision,
  platform,
}: {
  preview: SurfacePreview | null;
  error: string | null;
  busy: boolean;
  onRender: (display: string | null) => void;
  onDisplayChange: () => void;
  onOpen: () => void;
  revision: number;
  platform?: Capabilities["platform"];
}) {
  const [outputs, setOutputs] = useState<Display[]>([]);
  const [display, setDisplay] = useState<string | null>(null);
  useEffect(() => { void api.surfaceDisplays().then(setOutputs, () => {}); }, []);
  useEffect(() => {
    const render = () => { if (!document.hidden) onRender(display); };
    render();
    const timer = window.setInterval(render, 2000);
    window.addEventListener("focus", render);
    document.addEventListener("visibilitychange", render);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener("focus", render);
      document.removeEventListener("visibilitychange", render);
      onDisplayChange();
    };
  }, [display, onRender, onDisplayChange, revision]);
  return (
    <div className="animate-page-in space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-[14px] font-semibold text-text">Live compositor preview</h2>
          <p className="mt-0.5 text-[12px] text-muted">
            {preview
              ? `${preview.width}×${preview.height} · ${preview.masked} windows or regions covered · ${preview.source}`
              : "Waiting for a preview. Use the controls below to set playback for the share window."}
          </p>
        </div>
        <div className="flex gap-2">
          <DisplayPicker displays={outputs} value={display} onChange={(value) => { onDisplayChange(); setDisplay(value); }} />
          <Button variant="secondary" size="sm" onClick={() => onRender(display)} disabled={busy} title="Capture a single filtered frame" className="h-8 text-[12px]">
            <RefreshCw className={cn("size-3.5", busy && "animate-spin")} />
            {preview ? "Re-render" : "Render preview"}
          </Button>
          <Button size="sm" onClick={onOpen} title="Open the shareable Surface window" className="h-8 text-[12px]">
            <ExternalLink className="size-3.5" />
            Open Surface window
          </Button>
        </div>
      </div>

      <SurfacePlayback nativeAvailable={platform === "windows" && api.IN_TAURI} />

      {error && <p role="alert" className="rounded-lg border border-danger/30 bg-panel p-3 text-sm text-danger">{error}</p>}

      <div className="overflow-hidden rounded-xl border border-border bg-panel">
        <div className="flex items-center justify-between border-b border-border px-4 py-2.5">
          <span className="flex items-center gap-2 text-[12px] text-muted">
            <AppWindow className="size-3.5 text-accent" />
            Composite stream
          </span>
          <Badge variant={preview && preview.masked ? "accent" : "default"} className="text-[10px]">
            {preview ? `${preview.masked} veiled` : "idle"}
          </Badge>
        </div>
        <div className="scan-overlay relative aspect-[16/10] bg-app">
          {preview?.png_base64 ? (
            <img
              src={`data:image/png;base64,${preview.png_base64}`}
              alt="Veil Surface preview"
              className="size-full object-contain"
            />
          ) : (
            <div className="flex h-full items-center justify-center font-mono text-[12px] text-faint">
              [ AWAITING COMPOSITOR FRAME ]
            </div>
          )}
          <div className="pointer-events-none absolute top-3 left-3 rounded border border-border bg-app/85 px-2 py-1 font-mono text-[10px] text-muted backdrop-blur">
            ┌ VEIL_SURFACE // PREVIEW ONLY
          </div>
          <div className="pointer-events-none absolute right-3 bottom-3 rounded border border-border bg-app/85 px-2 py-1 font-mono text-[10px] text-accent backdrop-blur">
            {preview ? "SNAPSHOT" : "NO FRAME"} ┘
          </div>
        </div>
      </div>

      <div className="grid gap-3 md:grid-cols-3">
        {[
          {
            title: "Share the Surface",
            text: "In OBS, Discord, Zoom, or your browser share dialog, select the dedicated OmniScreenVeil Surface window — not your whole desktop.",
          },
          {
            title: "Stay in control locally",
            text: "Surface rules leave windows usable locally. macOS filters them from this app’s capture; other backends mask them. Other apps’ desktop captures are unchanged.",
          },
          {
            title: "Test before you share",
            text: "Compare real Surface frames in Capture test, then check the result in your actual sharing application before exposing sensitive content.",
          },
        ].map((item, i) => (
          <div key={item.title} className="rounded-lg border border-border bg-panel p-3.5">
            <span className="font-mono text-[10px] text-faint">0{i + 1}</span>
            <h3 className="mt-1 text-[12px] font-semibold text-text">{item.title}</h3>
            <p className="mt-1 text-[11px] leading-relaxed text-muted">{item.text}</p>
          </div>
        ))}
      </div>
    </div>
  );
}

function SurfacePlayback({ nativeAvailable }: { nativeAvailable: boolean }) {
  const [prefs, setPrefs] = useState<DesktopPreferences | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => { void api.getPreferences().then(setPrefs, (err) => setError(String(err))); }, []);

  async function change(preview_mode: DesktopPreferences["preview_mode"]) {
    if (!prefs) return;
    const next = { ...prefs, preview_mode };
    setSaving(true);
    setError(null);
    try { await api.savePreferences(next); setPrefs(next); }
    catch (err) { setError(String(err)); }
    finally { setSaving(false); }
  }

  return <section className="rounded-xl border border-border bg-panel p-3">
    <label className="flex flex-wrap items-center justify-between gap-2 text-xs">
      <span><strong className="text-text">Surface playback</strong><small className="mt-1 block text-muted">For the separate share window. {nativeAvailable ? "Native streaming uses one display. " : ""}Playback pauses when minimized.</small></span>
      <select className="rounded-md border border-border bg-app px-2.5 py-1.5 text-text" value={prefs?.preview_mode ?? "economy"} disabled={!prefs || saving} onChange={(e) => void change(e.target.value as DesktopPreferences["preview_mode"])}>
        <option value="economy">Snapshots · up to 1920 px</option>
        <option value="motion">Faster snapshots · up to 960 px</option>
        {(nativeAvailable || prefs?.preview_mode === "native") && <option value="native" disabled={!nativeAvailable}>Native stream (Windows) · up to 960 px</option>}
        <option value="manual">Manual refresh</option>
      </select>
    </label>
    {error && <p role="alert" className="mt-2 text-xs text-danger">{error}</p>}
  </section>;
}

export function AuditPage({ armed, live }: { armed: boolean; live: boolean }) {
  const [frames, setFrames] = useState<{ off?: SurfacePreview; on?: SurfacePreview }>({});
  const [error, setError] = useState<string | null>(null);
  const [capturing, setCapturing] = useState(false);
  const [displays, setDisplays] = useState<Display[]>([]);
  const [display, setDisplay] = useState<string | null>(null);
  useEffect(() => { void api.surfaceDisplays().then(setDisplays, () => {}); }, []);
  async function capture() {
    setCapturing(true);
    setError(null);
    try {
      const frame = await api.preview(display, false, true);
      if (frame.armed !== armed) throw new Error("Protection changed during this step. Capture again.");
      setFrames((old) => ({ ...old, [armed ? "on" : "off"]: frame }));
    } catch (err) { setError(String(err)); }
    finally { setCapturing(false); }
  }
  return <div className="animate-page-in space-y-4">
    <section className="rounded-xl border border-warning/25 bg-warning-soft px-4 py-3 text-[12px] text-muted">
      <strong className="text-warning">Use a harmless test window and stop any active share first.</strong>{" "}
      Place the test window where it is visible, save a Surface rule for it, then capture with protection off and on. Move OmniScreenVeil out of the test window’s way. Compare the two real captures below. They stay in this page and are not saved or uploaded.
      This tests the Veil Surface, not OBS, Zoom, or another app’s capture path.
    </section>
    <div className="flex flex-wrap items-center gap-2">
      <fieldset disabled={capturing}><DisplayPicker displays={displays} value={display} onChange={(value) => { setDisplay(value); setFrames({}); }} /></fieldset>
      <Button onClick={() => void capture()} disabled={!live || capturing} title="Take a real Surface snapshot with the current protection state">
        {capturing ? "Capturing…" : armed ? "Capture with protection on" : "Capture with protection off"}
      </Button>
      <Button variant="ghost" disabled={capturing || (!frames.off && !frames.on)} onClick={() => setFrames({})} title="Discard both captures">Clear captures</Button>
    </div>
    {!live && <p className="text-xs text-muted">A live desktop backend is required. No simulated result is shown.</p>}
    {error && <p role="alert" className="text-sm text-danger">{error}</p>}
    <div className="grid gap-3 lg:grid-cols-2">
      {([ ["Protection off", frames.off], ["Protection on", frames.on] ] as const).map(([label, frame]) => <section key={label} className="overflow-hidden rounded-xl border border-border bg-panel">
        <h3 className="px-3 py-2 text-sm font-medium">{label}</h3>
        {frame ? <><img className="w-full bg-black" src={`data:image/png;base64,${frame.png_base64}`} alt={`Live capture: ${label}`} /><p className="px-3 py-2 text-xs text-muted">{frame.masked} masked · {frame.source}</p></> : <p className="px-3 py-8 text-center text-xs text-muted">Not captured yet</p>}
      </section>)}
    </div>
  </div>;
}

/* ------------------------------ Rules ------------------------------------ */
const AUTO_MODES: { id: AutoArm; label: string; hint: string }[] = [
  { id: "off", label: "Manual", hint: "Arm with the button or Ctrl+Shift+V" },
  { id: "capture_app_running", label: "Capture app", hint: "Auto-arm when OBS, Zoom, Discord, etc. run" },
  { id: "always", label: "Always", hint: "Keep protection active continuously" },
];

function appLabel(value: string) {
  const parts = value.replaceAll("\\", "/").split("/");
  return parts[parts.length - 1] || value;
}

export function RulesPage({
  profile,
  windows,
  automationPaused,
  caps,
  busy,
  onAdd,
  onAutoArm,
  onToggleRule,
  onRemoveRule,
  onClear,
}: {
  profile: Profile | null;
  windows: WindowInfo[];
  automationPaused: boolean;
  caps: Capabilities | null;
  busy: boolean;
  onAdd: (app: AppKey, mode: ProtectionMode, enabled: boolean) => void;
  onAutoArm: (v: AutoArm) => void;
  onToggleRule: (id: number, enabled: boolean) => void;
  onRemoveRule: (id: number) => void;
  onClear: () => void;
}) {
  const rules = profile?.rules ?? [];
  const active = profile?.auto_arm ?? "off";
  const apps = [...new Map(windows.filter((w) => w.top_level).map((w) => [w.app.value.toLowerCase(), w])).values()].sort((a, b) => a.app_name.localeCompare(b.app_name));
  const [selection, setSelection] = useState("");
  const [identity, setIdentity] = useState("");
  const [mode, setMode] = useState<ProtectionMode>("surface");
  const [enabled, setEnabled] = useState(true);
  const platform = caps?.platform;
  const kind = platform === "windows" ? "windows_executable" : platform === "mac_os" ? "mac_bundle" : "linux_app_id";
  const hint = kind === "windows_executable" ? "Full executable path" : kind === "mac_bundle" ? "Bundle ID or .app path" : "Application ID or WM_CLASS";
  const examples = kind === "windows_executable" ? ["C:\\Apps\\Example\\Example.exe", "C:\\Apps\\Demo\\Demo.exe"] : kind === "mac_bundle" ? ["com.apple.TextEdit", "com.apple.calculator"] : ["org.gnome.Calculator", "org.gnome.TextEditor"];
  const field = "w-full rounded-md border border-border bg-app px-2.5 py-1.5 text-[12px] text-text";
  const chosenApp = apps.find((w) => w.app.value === selection)?.app;
  const manualApp: AppKey = { kind, value: identity.trim() };
  const hasApp = (app: AppKey) => rules.some((r) => r.target.kind === "app" && r.target.value.kind === app.kind && r.target.value.value.replaceAll("\\", "/").toLowerCase() === app.value.replaceAll("\\", "/").toLowerCase());

  return (
    <div className="animate-page-in space-y-4">
      <div className="rounded-xl border border-border bg-panel p-4">
        <div className="flex items-center justify-between">
          <div>
            <h3 className="text-[13px] font-semibold text-text">Auto-arm policy</h3>
            <p className="mt-0.5 text-[12px] text-muted">
              {AUTO_MODES.find((m) => m.id === active)?.hint}
            </p>
          </div>
        </div>
        <div className="mt-3 grid grid-cols-3 gap-2">
          {AUTO_MODES.map((m) => (
            <button
              key={m.id}
              type="button"
              disabled={busy}
              onClick={() => onAutoArm(m.id)}
              aria-pressed={active === m.id}
              title={m.hint}
              className={cn(
                "rounded-lg border px-3 py-2.5 text-left transition-all disabled:opacity-50",
                active === m.id
                  ? "border-accent/50 bg-accent-soft"
                  : "border-border bg-app hover:border-border-strong",
              )}
            >
              <span className={cn("block text-[12px] font-medium", active === m.id ? "text-accent" : "text-text")}>
                {m.label}
              </span>
              <span className="mt-0.5 block text-[10px] text-faint">{m.hint}</span>
            </button>
          ))}
        </div>
        <p className="mt-2 text-[11px] text-faint">
          {automationPaused ? "Automation is paused for this session. Arm protection to resume it." : "A running capture app is a hint, not proof of a live share. Disarming pauses auto-arm without changing this policy."}
        </p>
      </div>

      <form className="space-y-3 rounded-xl border border-border bg-panel p-4" onSubmit={(e) => {
        e.preventDefault();
        if (chosenApp || (!selection && manualApp.value)) onAdd(chosenApp ?? manualApp, mode, enabled);
      }}>
        <div><h3 className="text-[13px] font-semibold">Add an app rule</h3><p className="text-[11px] text-muted">App rules follow all of an app’s windows. Saving a rule for the same app replaces its current rule.</p></div>
        <div className="grid gap-3 sm:grid-cols-2">
          <label className="space-y-1 text-xs">Application<select className={field} value={selection} onChange={(e) => setSelection(e.target.value)}>
            <option value="">Enter an app manually</option>
            {apps.map((w) => <option value={w.app.value} key={w.app.value}>{w.app_name} · {w.app.value}</option>)}
          </select></label>
          {!selection && <label className="space-y-1 text-xs">{hint}<input required className={field} value={identity} onChange={(e) => setIdentity(e.target.value)} placeholder={examples[0]} /></label>}
          <label className="space-y-1 text-xs">Protection mode<select className={field} value={mode} onChange={(e) => setMode(e.target.value as ProtectionMode)}>
            {MODES.map((item) => <option key={item.id} value={item.id} disabled={caps?.modes[item.id].level === "unsupported"}>{item.label}</option>)}
          </select></label>
        </div>
        <p className="text-xs text-muted">{MODES.find((item) => item.id === mode)?.blurb}</p>
        <div className="flex flex-wrap items-center gap-3">
          <label className="flex items-center gap-2 text-xs"><Switch checked={enabled} onCheckedChange={setEnabled} aria-label="Enable this rule" />Enabled</label>
          <Button type="submit" disabled={busy || !profile || !caps || (!chosenApp && (Boolean(selection) || !manualApp.value)) || caps.modes[mode].level === "unsupported"} title="Save the rule without changing whether protection is armed">Save rule</Button>
        </div>
        <div className="flex flex-wrap items-center gap-2 text-xs text-muted">
          Disabled examples:
          {examples.map((value) => <Button key={value} type="button" size="sm" variant="secondary" disabled={busy || !profile || !caps || caps.modes.surface.level === "unsupported" || hasApp({ kind, value })} title={`Add ${value} as a disabled Surface rule`} onClick={() => onAdd({ kind, value }, "surface", false)}>{appLabel(value)}</Button>)}
        </div>
      </form>

      <div className="overflow-hidden rounded-xl border border-border bg-panel">
        <div className="flex items-center justify-between border-b border-border px-4 py-3">
          <div>
            <h3 className="text-[13px] font-semibold text-text">Persistent rules</h3>
            <p className="mt-0.5 text-[11px] text-faint">
              {rules.length === 0 ? "No rules stored" : `${rules.length} rule${rules.length === 1 ? "" : "s"} · ${rules.filter((r) => r.enabled).length} active`}
            </p>
          </div>
          {rules.length > 0 && (
            <Button variant="ghost" size="sm" onClick={onClear} disabled={busy} className="h-7 text-[11px] text-muted hover:text-danger">
              <Trash2 className="size-3" />
              Purge all
            </Button>
          )}
        </div>
        {rules.length === 0 ? (
          <div className="flex flex-col items-center gap-2 px-4 py-12 text-center">
            <ShieldCheck className="size-5 text-faint" />
            <p className="text-[12px] font-medium text-text">No persistent rules yet</p>
            <p className="max-w-[300px] text-[11px] text-faint">
              Toggle protection for a window in the Windows page to create a rule that survives restarts.
            </p>
          </div>
        ) : (
          <div className="divide-y divide-border/70">
            {rules.map((rule) => (
              <div key={rule.id} className="flex items-center gap-3 px-4 py-2.5">
                <ShieldCheck className={cn("size-4 shrink-0", rule.enabled ? "text-success" : "text-faint")} />
                <div className="min-w-0 flex-1">
                  <p className="truncate text-[12px] font-medium text-text">
                    {rule.target.kind === "app"
                      ? appLabel(rule.target.value.value)
                      : `Window 0x${rule.target.value.toString(16)}`}
                  </p>
                  <p className="truncate font-mono text-[10px] text-faint">
                    {rule.target.kind === "app" ? rule.target.value.value : "one-shot window handle"}
                  </p>
                </div>
                <Badge
                  variant={rule.mode === "exclusion" ? "live" : rule.mode === "surface" ? "accent" : "sim"}
                  className="text-[10px]"
                >
                  {modeLabel(rule.mode)}
                </Badge>
                <Switch
                  checked={rule.enabled}
                  disabled={busy}
                  onCheckedChange={(v) => onToggleRule(rule.id, v)}
                  aria-label={`Enable rule ${rule.id}`}
                  title={rule.enabled ? "Disable this rule without deleting it" : "Enable this rule"}
                />
                <button
                  type="button"
                  onClick={() => onRemoveRule(rule.id)}
                  title="Remove this saved rule"
                  aria-label={`Remove rule ${rule.id}`}
                  disabled={busy}
                  className="text-faint transition-colors hover:text-danger"
                >
                  <Trash2 className="size-3.5" />
                </button>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

/* --------------------------- Capabilities -------------------------------- */
export function CapabilitiesPage({
  caps,
  matrix,
}: {
  caps: Capabilities | null;
  matrix: Capabilities[];
}) {
  return (
    <div className="animate-page-in space-y-4">
      <div className="grid gap-3 md:grid-cols-3">
        {MODES.map((mode) => {
          const level = caps?.modes[mode.id];
          return (
            <div key={mode.id} className="rounded-xl border border-border bg-panel p-4">
              <div className="flex items-center justify-between">
                <span className="text-[13px] font-semibold text-text">{mode.label}</span>
                <Badge
                  variant={
                    level?.level === "native" ? "live" : level?.level === "partial" ? "sim" : "default"
                  }
                  className="text-[10px] capitalize"
                >
                  {level?.level ?? "…"}
                </Badge>
              </div>
              <p className="mt-2 text-[11px] leading-relaxed text-muted">
                {level ? levelDetail(level) : "Detecting…"}
              </p>
            </div>
          );
        })}
      </div>

      <div className="overflow-hidden rounded-xl border border-border bg-panel">
        <div className="border-b border-border px-4 py-3">
          <h3 className="text-[13px] font-semibold text-text">Cross-platform support matrix</h3>
          <p className="mt-0.5 text-[11px] text-faint">
            ✓ native · ◐ partial · — unsupported. No capability is claimed without a test.
          </p>
        </div>
        <table className="w-full text-[12px]">
          <thead>
            <tr className="border-b border-border text-left text-[10px] uppercase tracking-wide text-faint">
              <th className="px-4 py-2 font-medium">Mode</th>
              {matrix.map((entry) => (
                <th key={entry.platform} className="px-3 py-2 text-center font-medium">
                  {PLATFORM_LABELS[entry.platform]}
                </th>
              ))}
            </tr>
          </thead>
          <tbody className="divide-y divide-border/70">
            {MODES.map((mode) => (
              <tr key={mode.id}>
                <td className="px-4 py-2.5 text-muted">{mode.label}</td>
                {matrix.map((entry) => {
                  const level = entry.modes[mode.id];
                  return (
                    <td
                      key={entry.platform}
                      className={cn(
                        "px-3 py-2.5 text-center font-semibold",
                        level.level === "native" && "text-success",
                        level.level === "partial" && "text-warning",
                        level.level === "unsupported" && "text-faint",
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
    </div>
  );
}
