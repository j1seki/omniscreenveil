import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import * as api from "../api";
import type { DesktopPreferences, Display, PreferenceChanges } from "../types";

export function useCaptureSettings() {
  const [settings, setSettings] = useState<DesktopPreferences | null>(null);
  const [displays, setDisplays] = useState<Display[]>([]);
  const [displayError, setDisplayError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const revision = useRef(0);

  const load = useCallback(async () => {
    const request = ++revision.current;
    try {
      const preferences = await api.getPreferences();
      if (request === revision.current) {
        setSettings(preferences);
        setError(null);
      }
    } catch (err) {
      if (request === revision.current) setError(String(err));
    }
  }, []);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    let displayRequest = 0;
    const updateDisplays = async () => {
      const request = ++displayRequest;
      try {
        const list = await api.surfaceDisplays();
        if (active && request === displayRequest) {
          setDisplays(list);
          setDisplayError(api.IN_TAURI && !list.length ? "No individual displays were detected." : null);
        }
      } catch (err) {
        if (active && request === displayRequest) setDisplayError(`Could not list displays: ${String(err)}`);
      }
    };
    const onStorage = (event: StorageEvent) => {
      if (event.key === api.PREVIEW_PREFERENCES_KEY) void load();
    };

    if (api.IN_TAURI) {
      void listen("veil://preferences", () => void load()).then((stop) => {
        if (active) { unlisten = stop; void load(); }
        else stop();
      }, (err) => { if (active) setError(String(err)); });
    } else {
      window.addEventListener("storage", onStorage);
    }
    const onFocus = () => { void updateDisplays(); void load(); };
    window.addEventListener("focus", onFocus);
    void updateDisplays();
    void load();
    return () => {
      active = false;
      ++revision.current;
      unlisten?.();
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("storage", onStorage);
    };
  }, [load]);

  async function save(changes: PreferenceChanges) {
    setSaving(true);
    setError(null);
    try {
      await api.savePreferences(changes);
      await load();
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }

  return { settings, displays, displayError, error, saving, save };
}
