import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import * as api from "../api";
import type { UpdateChannel, UpdateInfo, UpdateProgress } from "../types";

const CHANNEL_KEY = "veil.update.channel";
const AUTO_KEY = "veil.update.autoCheck";
const LAST_KEY = "veil.update.lastChecked";
const NOTIFIED_PREFIX = "veil.update.notified.";
const AUTO_INTERVAL_MS = 6 * 60 * 60 * 1000;

type Notice = (message: string, kind?: "success" | "info" | "error") => void;

function readChannel(): UpdateChannel {
  if (typeof window === "undefined") return "stable";
  return window.localStorage.getItem(CHANNEL_KEY) === "beta" ? "beta" : "stable";
}

function readAutoCheck(): boolean {
  if (typeof window === "undefined") return true;
  return window.localStorage.getItem(AUTO_KEY) !== "false";
}

function readLastChecked(): number | null {
  if (typeof window === "undefined") return null;
  const value = Number(window.localStorage.getItem(LAST_KEY));
  return Number.isFinite(value) && value > 0 ? value : null;
}

/** Turn updater transport errors into something the UI can show honestly. */
export function friendlyUpdateError(error: string): string {
  const lower = error.toLowerCase();
  if (
    lower.includes("could not fetch a valid release json") ||
    lower.includes("release not found") ||
    lower.includes("404")
  ) {
    return "No update manifest is published for this channel yet.";
  }
  if (
    lower.includes("error sending request") ||
    lower.includes("dns") ||
    lower.includes("connection") ||
    lower.includes("timed out")
  ) {
    return "Could not reach GitHub. Check your network connection and try again.";
  }
  return error;
}

/**
 * Update state shared by the app shell and the Updates page.
 *
 * The channel is stored in localStorage rather than the protection profile:
 * it is a local preference, not protection policy. Only the check is
 * automatic; installing always requires an explicit user action.
 */
export function useUpdater(notify: Notice) {
  const [currentVersion, setCurrentVersion] = useState("");
  const [channel, setChannelState] = useState<UpdateChannel>(readChannel);
  const [autoCheck, setAutoCheckState] = useState<boolean>(readAutoCheck);
  const [lastChecked, setLastChecked] = useState<number | null>(readLastChecked);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [manifestFound, setManifestFound] = useState<boolean | null>(null);
  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [progress, setProgress] = useState<UpdateProgress | null>(null);
  const [installedVersion, setInstalledVersion] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const channelRef = useRef(channel);
  channelRef.current = channel;
  const checkingRef = useRef(false);
  const installedRef = useRef<string | null>(null);
  const notifyRef = useRef(notify);
  notifyRef.current = notify;

  useEffect(() => {
    let cancelled = false;
    void api
      .appVersion()
      .then((version) => {
        if (!cancelled) setCurrentVersion(version);
      })
      .catch(() => {
        if (!cancelled) setCurrentVersion("unknown");
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!api.IN_TAURI) return;
    let unlisten: (() => void) | undefined;
    void (async () => {
      unlisten = await listen<UpdateProgress>("veil://update-progress", (event) => {
        setProgress(event.payload);
      });
    })();
    return () => unlisten?.();
  }, []);

  const check = useCallback(async (mode: "manual" | "auto" = "manual") => {
    if (!api.IN_TAURI) {
      if (mode === "manual") {
        notifyRef.current("Update checks are only available in the desktop app.", "info");
      }
      return null;
    }
    if (checkingRef.current || installedRef.current) return null;

    const requestedChannel = channelRef.current;
    checkingRef.current = true;
    setChecking(true);
    setError(null);
    try {
      const result = await api.checkForUpdate(requestedChannel);
      // The user may have switched channels while the request was in flight.
      if (channelRef.current !== requestedChannel) return null;

      const now = Date.now();
      setUpdate(result.update);
      setManifestFound(result.manifest_found);
      setLastChecked(now);
      window.localStorage.setItem(LAST_KEY, String(now));

      if (result.update) {
        if (mode === "manual") {
          notifyRef.current(`Version ${result.update.version} is available.`, "info");
        } else {
          const key = `${NOTIFIED_PREFIX}${result.update.version}`;
          if (window.localStorage.getItem(key) !== "1") {
            window.localStorage.setItem(key, "1");
            notifyRef.current(
              `OmniScreenVeil ${result.update.version} is available.`,
              "info",
            );
          }
        }
      } else if (mode === "manual") {
        notifyRef.current(
          result.manifest_found
            ? "You are on the latest version for this channel."
            : "No update manifest is published for this channel yet.",
          "info",
        );
      }
      return result.update;
    } catch (err) {
      const message = friendlyUpdateError(String(err));
      setError(message);
      if (mode === "manual") notifyRef.current(message, "error");
      return null;
    } finally {
      checkingRef.current = false;
      setChecking(false);
    }
  }, []);

  // Automatic checks look for updates; they never download or install one.
  useEffect(() => {
    if (!api.IN_TAURI || !autoCheck || installedVersion) return;
    const initial = window.setTimeout(() => {
      void check("auto");
    }, 5000);
    const interval = window.setInterval(() => {
      void check("auto");
    }, AUTO_INTERVAL_MS);
    return () => {
      window.clearTimeout(initial);
      window.clearInterval(interval);
    };
  }, [autoCheck, installedVersion, channel, check]);

  const install = useCallback(async () => {
    if (!api.IN_TAURI) {
      notifyRef.current("Updates can only be installed in the desktop app.", "info");
      return;
    }
    if (!update || installing) return;

    setInstalling(true);
    setError(null);
    setProgress({ phase: "downloading", downloaded: 0, total: null });
    try {
      const result = await api.installUpdate(channelRef.current);
      installedRef.current = result.version;
      setInstalledVersion(result.version);
      setUpdate(null);
      setProgress({ phase: "installed", downloaded: 0, total: null });
      notifyRef.current(
        `OmniScreenVeil ${result.version} installed. Restart to finish.`,
        "success",
      );
    } catch (err) {
      const message = friendlyUpdateError(String(err));
      setError(message);
      setProgress(null);
      notifyRef.current(message, "error");
    } finally {
      setInstalling(false);
    }
  }, [update, installing]);

  const setChannel = useCallback((next: UpdateChannel) => {
    channelRef.current = next;
    setChannelState(next);
    window.localStorage.setItem(CHANNEL_KEY, next);
    setUpdate(null);
    setManifestFound(null);
    setProgress(null);
    setError(null);
  }, []);

  const setAutoCheck = useCallback((next: boolean) => {
    setAutoCheckState(next);
    window.localStorage.setItem(AUTO_KEY, String(next));
  }, []);

  const restart = useCallback(async () => {
    if (!api.IN_TAURI) return;
    try {
      await api.restartApp();
    } catch (err) {
      notifyRef.current(String(err), "error");
    }
  }, []);

  return {
    supported: api.IN_TAURI,
    currentVersion,
    channel,
    setChannel,
    autoCheck,
    setAutoCheck,
    lastChecked,
    update,
    manifestFound,
    checking,
    installing,
    progress,
    installedVersion,
    error,
    check,
    install,
    restart,
  };
}
