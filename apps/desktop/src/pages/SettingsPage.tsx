import { useEffect, useState } from "react";
import * as api from "../api";
import type { DesktopPreferences, Theme } from "../types";
import { Switch } from "../components/ui/switch";

export function SettingsPage({ theme, onToggleTheme }: { theme: Theme; onToggleTheme: () => void }) {
  const [prefs, setPrefs] = useState<DesktopPreferences | null>(null);
  const [autostart, setAutostart] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    void api.getPreferences().then(setPrefs, (err) => setError(String(err)));
    if (api.IN_TAURI) void api.loginStartup().then(setAutostart, (err) => setError(String(err)));
  }, []);

  async function save(next: DesktopPreferences) {
    setBusy(true); setError(null);
    try { await api.savePreferences(next); setPrefs(next); }
    catch (err) { setError(String(err)); }
    finally { setBusy(false); }
  }

  async function toggleStartup(enabled: boolean) {
    setBusy(true); setError(null);
    try { setAutostart(await api.loginStartup(enabled)); }
    catch (err) { setError(String(err)); }
    finally { setBusy(false); }
  }

  const selectClass = "rounded-md border border-border bg-app px-2 py-1.5 text-text";
  return <div className="animate-page-in space-y-4 text-[13px]">
    {error && <p role="alert" className="text-danger">{error}</p>}
    <section className="divide-y divide-border rounded-xl border border-border bg-panel px-4">
      <label className="flex items-center justify-between gap-4 py-3">
        <span>Launch at login <small className="block text-muted">Start OmniScreenVeil when you sign in.</small></span>
        <Switch checked={autostart ?? false} disabled={busy || autostart === null} onCheckedChange={(value) => void toggleStartup(value)} aria-label="Launch at login" />
      </label>
      <label className="flex items-center justify-between gap-4 py-3">
        <span>When closing the main window <small className="block text-muted">Minimize keeps protection running. Quit releases it.</small></span>
        <select className={selectClass} disabled={!prefs || busy} value={prefs?.close_behavior ?? "quit"} onChange={(e) => prefs && void save({ ...prefs, close_behavior: e.target.value as DesktopPreferences["close_behavior"] })}>
          <option value="quit">Quit app</option><option value="minimize">Minimize</option>
        </select>
      </label>
      <label className="flex items-center justify-between gap-4 py-3">
        <span>Surface playback <small className="block text-muted">Faster uses smaller frames. All modes pause when the Surface is minimized.</small></span>
        <select className={selectClass} disabled={!prefs || busy} value={prefs?.preview_mode ?? "economy"} onChange={(e) => prefs && void save({ ...prefs, preview_mode: e.target.value as DesktopPreferences["preview_mode"] })}>
          <option value="economy">Snapshots · up to 1920 px</option>
          <option value="motion">Faster preview · up to 960 px</option>
          <option value="manual">Manual refresh</option>
        </select>
      </label>
      <label className="flex items-center justify-between gap-4 py-3">
        <span>Dark appearance</span>
        <Switch checked={theme === "dark"} onCheckedChange={onToggleTheme} aria-label="Dark appearance" />
      </label>
    </section>
  </div>;
}
