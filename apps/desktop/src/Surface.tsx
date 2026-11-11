import { useEffect, useState } from "react";
import * as api from "./api";

/// Clean, chrome-free window that an audience can be shown.
///
/// It is deliberately not the main window: no controls, no window list, no
/// capability copy. It only polls the compositor and paints the latest frame.
/// The Surface window itself is masked out of that frame by the backend, so it
/// cannot recurse into its own output.
export default function Surface() {
  const [src, setSrc] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    let timer: number | undefined;

    const poll = async () => {
      try {
        const frame = await api.preview();
        if (!alive) return;
        setSrc(`data:image/png;base64,${frame.png_base64}`);
        setError(null);
      } catch (err) {
        if (alive) setError(String(err));
      } finally {
        if (alive) timer = window.setTimeout(poll, 1500);
      }
    };

    void poll();
    return () => {
      alive = false;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, []);

  return (
    <div className="surface">
      {src ? (
        <img src={src} alt="Veil Surface: protected windows are masked" />
      ) : (
        <div className="surface-status">Waiting for the first frame…</div>
      )}
      {error && <div className="surface-error">{error}</div>}
    </div>
  );
}
