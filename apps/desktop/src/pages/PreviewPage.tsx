import { DisplayPicker } from "../components/DisplayPicker";
import { PreviewPanel, type PreviewPanelProps } from "../components/PreviewPanel";
import type { useCaptureSettings } from "../lib/captureSettings";
import type { PreviewSettings, StreamSettings } from "../types";

const SELECT = "rounded-md border border-border bg-app px-2.5 py-1.5 text-text disabled:opacity-50";
const RESOLUTIONS = [320, 640, 960, 1280, 1920, 2560, 3840];
const FRAME_RATES = [1, 5, 10, 12, 15, 24, 30, 60];
const PREVIEW_INTERVALS = [250, 500, 1000, 1500, 2000, 3000, 5000, 10_000];

function NumberSetting({ label, value, options, format, onChange }: {
  label: string;
  value: number;
  options: number[];
  format: (value: number) => string;
  onChange: (value: number) => void;
}) {
  const choices = options.includes(value) ? options : [...options, value].sort((a, b) => a - b);
  return (
    <label className="flex flex-col gap-1.5">
      {label}
      <select className={SELECT} value={value} onChange={(e) => onChange(Number(e.target.value))}>
        {choices.map((choice) => <option key={choice} value={choice}>{format(choice)}</option>)}
      </select>
    </label>
  );
}

export function PreviewPage({ preferences, preview, nativeAvailable }: {
  preferences: ReturnType<typeof useCaptureSettings>;
  preview: PreviewPanelProps;
  nativeAvailable: boolean;
}) {
  const { settings, displays, displayError, saving, error, save } = preferences;
  const stream = settings?.stream;
  const playback = settings?.preview;
  const mode = stream?.mode === "native" && nativeAvailable ? "native" : "snapshot";
  const intervalLabel = (value: number) => value < 1000 ? `${value} ms` : `${value / 1000} s`;
  const edgeLabel = (value: number) => `${value} px long edge`;

  return (
    <div className="animate-page-in space-y-4">
      {playback && (
        <fieldset disabled={saving} className="rounded-xl border border-border bg-panel p-4 text-[12px]">
          <legend className="px-1.5 text-text">Preview playback</legend>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <label className="flex flex-col gap-1.5">
              Playback
              <select className={SELECT} value={playback.playback} onChange={(e) => void save({ preview: { playback: e.target.value as PreviewSettings["playback"] } })}>
                <option value="automatic">Automatic snapshots</option>
                <option value="manual">Manual</option>
              </select>
            </label>
            <div className="flex items-end [&_label]:w-full [&_label]:flex-col [&_label]:items-stretch [&_label]:gap-1.5">
              <DisplayPicker label="Preview display" displays={displays} value={playback.display || null} onChange={(id) => void save({ preview: { display: id ?? "" } })} />
            </div>
            <NumberSetting label="Preview resolution" value={playback.max_edge} options={RESOLUTIONS} format={edgeLabel} onChange={(max_edge) => void save({ preview: { max_edge } })} />
            {playback.playback === "automatic" && <NumberSetting label="Preview interval" value={playback.interval_ms} options={PREVIEW_INTERVALS} format={intervalLabel} onChange={(interval_ms) => void save({ preview: { interval_ms } })} />}
          </div>
          <p className="mt-2 text-[11px] text-muted">Applies to the dashboard preview here and in Overview.</p>
        </fieldset>
      )}
      {stream && (
        <fieldset disabled={saving} className="rounded-xl border border-border bg-panel p-4 text-[12px]">
          <legend className="px-1.5 text-text">Stream quality</legend>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <label className="flex flex-col gap-1.5">
              Capture method
              <select className={SELECT} value={mode} onChange={(e) => void save({ stream: { mode: e.target.value as StreamSettings["mode"] } })}>
                <option value="native" disabled={!nativeAvailable}>Native capture (Windows)</option>
                <option value="snapshot">Compatible capture</option>
              </select>
            </label>
            <div className="flex items-end [&_label]:w-full [&_label]:flex-col [&_label]:items-stretch [&_label]:gap-1.5">
              <DisplayPicker label="Stream display" displays={displays} value={stream.display || null} onChange={(id) => void save({ stream: { display: id ?? "" } })} />
            </div>
            <NumberSetting label="Output resolution" value={stream.max_edge} options={RESOLUTIONS} format={edgeLabel} onChange={(max_edge) => void save({ stream: { max_edge } })} />
            <NumberSetting label="Target frame rate" value={stream.fps} options={FRAME_RATES} format={(value) => `${value} fps`} onChange={(fps) => void save({ stream: { fps } })} />
          </div>
          <p className="mt-2 text-[11px] text-muted">Stream starts automatically when protection is armed and a Surface rule is enabled. Select OmniScreenVeil Stream in your sharing app.</p>
          {nativeAvailable && <p className="mt-1 text-[11px] text-muted">Keep the Stream window behind your work or on another display to avoid capturing the viewer in its own feed.</p>}
          {mode === "native" && !stream.display && <p className="mt-1 text-[11px] text-muted">All displays uses compatible capture. Choose one display to use native capture.</p>}
        </fieldset>
      )}
      {error && <p role="alert" className="text-sm text-danger">{error}</p>}
      {displayError && <p role="status" className="text-sm text-warning">{displayError}</p>}
      <PreviewPanel {...preview} />
    </div>
  );
}
