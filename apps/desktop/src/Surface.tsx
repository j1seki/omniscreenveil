import { useEffect, useState } from "react";
import * as api from "./api";
import { CaptureImage } from "./components/CaptureImage";
import { useCapturePlayback } from "./lib/capturePlayback";
import { useCaptureSettings } from "./lib/captureSettings";

export default function Surface() {
  const [platformReady, setPlatformReady] = useState(false);
  const [nativeAvailable, setNativeAvailable] = useState(false);
  const [viewerError, setViewerError] = useState<string | null>(null);
  const { settings, error } = useCaptureSettings();
  const stream = settings?.stream;
  const display = stream?.display || null;
  const native = stream?.mode === "native" && nativeAvailable && Boolean(display);
  const { frame, message } = useCapturePlayback({
    enabled: platformReady && Boolean(stream),
    background: true,
    display,
    edge: stream?.max_edge ?? 1280,
    fps: native ? stream?.fps : undefined,
    interval: !native && stream ? 1000 / stream.fps : undefined,
  });

  useEffect(() => {
    let active = true;
    void api.capabilities().then((caps) => {
      if (!active) return;
      setNativeAvailable(api.IN_TAURI && caps.platform === "windows");
      setPlatformReady(true);
    }, (err) => { if (active) setViewerError(String(err)); });
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") void api.closeSurface().catch((err) => setViewerError(String(err)));
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
      active = false;
      window.removeEventListener("keydown", onKeyDown);
    };
  }, []);

  return <div className="surface">
    {frame
      ? <CaptureImage image={frame.image} alt="OmniScreenVeil Stream output" />
      : <div className="surface-status px-8 text-center" role="status">{viewerError ?? error ?? message}</div>}
  </div>;
}
