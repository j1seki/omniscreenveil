import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import * as api from "../api";
import type { CaptureFrame } from "../types";
import { playNativeStream, waitForFrame } from "./nativeStream";

const UNSTABLE_CAPTURE = "The preview layout changed during capture. Waiting for a stable frame.";

interface PlaybackOptions {
  enabled: boolean;
  display: string | null;
  edge: number;
  interval?: number;
  fps?: number;
  background?: boolean;
}

export function useCapturePlayback({ enabled, display, edge, interval, fps, background = false }: PlaybackOptions) {
  const [frame, setFrame] = useState<CaptureFrame | null>(null);
  const [message, setMessage] = useState("Loading capture settings…");
  const [generation, setGeneration] = useState(0);
  const queue = useRef<Promise<void>>(Promise.resolve());
  const captureManually = useRef(false);
  const running = useRef<AbortController>();

  const invalidate = useCallback((manual = false) => {
    running.current?.abort();
    captureManually.current = manual;
    setFrame(null);
    setGeneration((value) => value + 1);
  }, []);

  useEffect(() => {
    if (!enabled) return;
    let active = true;
    let unlisten: (() => void) | undefined;
    if (api.IN_TAURI) {
      void listen("veil://changed", () => invalidate()).then((stop) => {
        if (active) unlisten = stop;
        else stop();
      }, () => {});
    }
    const visibility = () => { if (!background) invalidate(); };
    document.addEventListener("visibilitychange", visibility);
    return () => {
      active = false;
      unlisten?.();
      document.removeEventListener("visibilitychange", visibility);
    };
  }, [enabled, background, invalidate]);

  useEffect(() => {
    const explicit = captureManually.current;
    captureManually.current = false;
    setFrame(null);
    if (!enabled || (!background && document.hidden)) return;
    if (!fps && !interval && !explicit) {
      setMessage("Select Capture frame to update the preview.");
      return;
    }
    const controller = new AbortController();
    const { signal } = controller;
    running.current = controller;
    setMessage("Waiting for a frame…");
    let lastError: string | undefined;

    function fail(err: unknown) {
      if (signal.aborted) return;
      const error = String(err);
      if (lastError !== error) api.recordLog("warn", error);
      lastError = error;
      setFrame(null);
      setMessage(error);
    }

    async function snapshot() {
      const next = await api.preview(display, edge);
      if (!signal.aborted) {
        setFrame(next);
        lastError = undefined;
      }
    }

    async function refreshNativeFrame() {
      try {
        await snapshot();
        return true;
      } catch (err) {
        if (String(err) === UNSTABLE_CAPTURE) return false;
        throw err;
      }
    }

    queue.current = queue.current.then(async () => {
      if (signal.aborted) return;
      if (fps && display) {
        while (!signal.aborted) {
          // Seed every session, including a reconnect after Windows hid the
          // viewer. DXGI can wait indefinitely on a static desktop.
          await snapshot().catch(() => {});
          if (signal.aborted) return;
          try {
            await playNativeStream(display, edge, fps, signal, (next) => {
              if (signal.aborted) return;
              setFrame(next);
              if (!next) setMessage("Waiting for an updated frame…");
              else lastError = undefined;
            }, refreshNativeFrame);
          } catch (err) {
            fail(err);
          }
          await waitForFrame(2000, signal);
        }
        return;
      }
      do {
        const start = performance.now();
        try {
          await snapshot();
        } catch (err) {
          if (!interval || String(err) !== UNSTABLE_CAPTURE) fail(err);
        }
        if (!interval) break;
        const delay = lastError ? Math.max(interval, 1500) : interval;
        await waitForFrame(delay - (performance.now() - start), signal);
      } while (!signal.aborted);
    }).catch(fail);

    return () => controller.abort();
  }, [enabled, background, display, edge, fps, interval, generation]);

  return { frame, message, invalidate, capture: () => invalidate(true) };
}
