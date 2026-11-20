import * as api from "../api";
import type { CaptureFrame } from "../types";

export function waitForFrame(delay: number, signal: AbortSignal) {
  return new Promise<void>((resolve) => {
    if (delay <= 0 || signal.aborted) { resolve(); return; }
    const done = () => {
      window.clearTimeout(timer);
      signal.removeEventListener("abort", done);
      resolve();
    };
    const timer = window.setTimeout(done, delay);
    signal.addEventListener("abort", done, { once: true });
  });
}

/** Keep one frame in flight and close the capture session before restarting it. */
export async function playNativeStream(
  monitor: string,
  maxEdge: number,
  fps: number,
  signal: AbortSignal,
  show: (frame: CaptureFrame | null) => void,
  refresh: () => Promise<boolean>,
) {
  const id = await api.openStream(monitor, maxEdge);
  let refreshPending = false;
  try {
    while (!signal.aborted) {
      const start = performance.now();
      const buffer = await api.nextStreamFrame(id);
      if (signal.aborted) break;
      if (buffer.byteLength === 1) {
        if (new Uint8Array(buffer)[0] === 0) show(null);
        refreshPending = true;
      } else if (buffer.byteLength > 1) {
        show(api.readCaptureFrame(buffer));
        refreshPending = false;
      }
      if (refreshPending) refreshPending = !(await refresh());
      await waitForFrame(1000 / fps - (performance.now() - start), signal);
    }
  } finally {
    await api.closeStream(id).catch(() => {});
  }
}
