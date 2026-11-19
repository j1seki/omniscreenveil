import { MODES, type Capabilities } from "../types";

const explanations = {
  exclusion: "Keeps the window visible to you and asks Windows to exclude it from capture. Test with the app you share from.",
  surface: "Keeps the window usable locally and masks it in Veil Surface. Share the Surface window, not your desktop.",
  strict_hide: "Hides the window on your desktop while armed. Disarm to restore it.",
};

export function ProtectionModesGuide({ caps }: { caps: Capabilities | null }) {
  return <section className="rounded-xl border border-border bg-panel px-4 py-3" aria-label="Protection modes">
    <h3 className="text-[13px] font-semibold text-text">Protection modes</h3>
    <dl className="mt-2 grid gap-x-5 gap-y-2 sm:grid-cols-3">
      {MODES.map(({ id, label }) => <div key={id}>
        <dt className="text-xs font-medium text-text">{label}{caps?.modes[id].level === "unsupported" && <span className="ml-1 text-faint">· unavailable here</span>}</dt>
        <dd className="mt-0.5 text-[11px] leading-relaxed text-muted">{explanations[id]}</dd>
      </div>)}
    </dl>
    {caps?.platform === "windows" && <p className="mt-2 text-[11px] text-faint">If you hide the OmniScreenVeil dashboard, use the tray menu to release protection or press Ctrl+Shift+V to disarm.</p>}
  </section>;
}
