import * as api from "../api";

/** One native frame in flight. Revoke buffers as soon as they're replaced. */
export function playNativeStream(monitor: string | null, show: (src: string | null) => void, fail: (error: string) => void) {
  let stopped = false;
  let id: number | undefined;
  let url: string | undefined;
  function paint(next?: string) {
    if (url) URL.revokeObjectURL(url);
    url = next;
    show(url ?? null);
  }
  void (async () => {
    try {
      id = await api.openStream(monitor);
      while (!stopped) {
        const start = performance.now();
        const buffer = await api.nextStreamFrame(id);
        if (stopped) break;
        if (buffer.byteLength === 1) paint();
        else if (buffer.byteLength > 1) paint(URL.createObjectURL(new Blob([buffer], { type: "image/png" })));
        await new Promise<void>((resolve) => window.setTimeout(resolve, Math.max(0, 83 - (performance.now() - start))));
      }
    } catch (err) {
      if (!stopped) { paint(); fail(String(err)); }
    } finally {
      if (id !== undefined) void api.closeStream(id).catch(() => {});
    }
  })();
  return () => {
    stopped = true;
    if (url) URL.revokeObjectURL(url);
    if (id !== undefined) void api.closeStream(id).catch(() => {});
  };
}
