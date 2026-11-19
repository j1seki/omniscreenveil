import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { RefreshCw, X } from "lucide-react";
import * as api from "./api";
import { DisplayPicker } from "./components/DisplayPicker";
import { playNativeStream } from "./lib/nativeStream";
import type { DesktopPreferences, Display } from "./types";

export default function Surface() {
  const [src, setSrc] = useState<string | null>(null);
  const [message, setMessage] = useState("Waiting for the first frame…");
  const [displays, setDisplays] = useState<Display[]>([]);
  const [monitor, setMonitor] = useState<string | null>(null);
  const [mode, setMode] = useState<DesktopPreferences["preview_mode"] | null>(null);
  const [revision, refresh] = useState(0);
  const inFlight = useRef<Promise<unknown> | null>(null);

  useEffect(() => {
    if (mode === "native" && !monitor && displays.length === 1) setMonitor(displays[0].id);
  }, [mode, monitor, displays]);

  useEffect(() => {
    let active = true;
    const cleanups: (() => void)[] = [];
    const updateDisplays = () => void api.surfaceDisplays().then((list) => { if (active) setDisplays(list); }, () => {});
    const change = () => { setSrc(null); refresh((r) => r + 1); };
    const close = (event: KeyboardEvent) => {
      if (event.key === "Escape") void api.closeSurface().catch((err) => setMessage(String(err)));
    };
    window.addEventListener("keydown", close);
    window.addEventListener("focus", updateDisplays);
    if (api.IN_TAURI) {
      void listen("veil://changed", change).then((stop) => active ? cleanups.push(stop) : stop()).catch(() => {});
      void listen<DesktopPreferences>("veil://preferences", (event) => {
        setMode(event.payload.preview_mode);
        change();
      }).then((stop) => active ? cleanups.push(stop) : stop()).catch(() => {});
    }
    updateDisplays();
    void api.getPreferences().then((prefs) => { if (active) setMode(prefs.preview_mode); }, () => { if (active) setMode("economy"); });
    return () => {
      active = false;
      window.removeEventListener("keydown", close);
      window.removeEventListener("focus", updateDisplays);
      cleanups.forEach((stop) => stop());
    };
  }, []);

  useEffect(() => {
    if (!mode) return;
    if (mode === "native") {
      let stop: (() => void) | undefined;
      let active = true;
      const sync = () => {
        stop?.(); stop = undefined; setSrc(null);
        if (document.hidden) return;
        if (!monitor) { setMessage("Choose one display to start native streaming."); return; }
        setMessage("Waiting for the first native frame…");
        stop = playNativeStream(monitor, setSrc, (error) => {
          stop = undefined;
          if (active) setMessage(`${error} Use the dashboard’s Surface → Surface playback to switch to snapshots.`);
        });
      };
      document.addEventListener("visibilitychange", sync);
      const focus = () => { if (!stop && !document.hidden) sync(); };
      window.addEventListener("focus", focus);
      sync();
      return () => { active = false; stop?.(); window.removeEventListener("focus", focus); document.removeEventListener("visibilitychange", sync); };
    }
    let active = true;
    let epoch = 0;
    let minimized = false;
    let timer: number | undefined;
    const poll = async () => {
      const began = performance.now();
      const version = epoch;
      try {
        if (document.hidden || (api.IN_TAURI && await getCurrentWindow().isMinimized().catch(() => false))) {
          minimized = true;
          if (active) { setSrc(null); setMessage("Capture paused while minimized"); }
          return;
        }
        minimized = false;
        if (inFlight.current) await inFlight.current.catch(() => {});
        if (!active || epoch !== version || document.hidden) return;
        const request = api.preview(monitor, mode === "motion");
        inFlight.current = request;
        try {
          const frame = await request;
          if (active && epoch === version && !document.hidden) setSrc(`data:image/png;base64,${frame.png_base64}`);
        } finally {
          if (inFlight.current === request) inFlight.current = null;
        }
      } catch (err) {
        if (active && epoch === version) { setSrc(null); setMessage(String(err)); }
      } finally {
        if (active && epoch === version && !document.hidden && mode !== "manual") {
          const period = mode === "motion" ? 200 : 1500;
          timer = window.setTimeout(poll, Math.max(40, period - (performance.now() - began)));
        }
      }
    };
    const visibility = () => {
      epoch++;
      window.clearTimeout(timer);
      setSrc(null);
      if (!document.hidden) void poll();
    };
    const focus = () => { if (minimized && !document.hidden) visibility(); };
    document.addEventListener("visibilitychange", visibility);
    window.addEventListener("focus", focus);
    void poll();
    return () => {
      active = false;
      window.clearTimeout(timer);
      document.removeEventListener("visibilitychange", visibility);
      window.removeEventListener("focus", focus);
    };
  }, [mode, monitor, revision]);

  return <div className="surface">
    {src ? <img src={src} alt="Veil Surface capture" /> : <div className="surface-status px-8 text-center" role="status">{message}</div>}
    <div className={`absolute top-0 right-0 left-0 flex items-center justify-end gap-3 bg-black/85 px-3 py-2 text-white transition-opacity hover:opacity-100 focus-within:opacity-100 ${src ? "opacity-0" : "opacity-100"}`}>
      <DisplayPicker displays={displays} value={monitor} onChange={(value) => { setSrc(null); setMessage("Switching displays…"); setMonitor(value); }} />
      <button type="button" title="Refresh the Surface frame" aria-label="Refresh the Surface frame" onClick={() => refresh((r) => r + 1)}><RefreshCw size={16} /></button>
      <button type="button" title="Close Surface (Esc)" aria-label="Close Surface" onClick={() => void api.closeSurface().catch((err) => setMessage(String(err)))}><X size={18} /></button>
    </div>
  </div>;
}
