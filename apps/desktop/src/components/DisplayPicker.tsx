import type { Display } from "../types";

export function DisplayPicker({ displays, value, onChange }: {
  displays: Display[];
  value: string | null;
  onChange: (value: string | null) => void;
}) {
  return <label className="inline-flex items-center gap-2 text-xs" title="Choose the desktop layout or capture a single display">
    Display
    <select className="rounded-md border border-border bg-panel px-2 py-1.5 text-text" value={value ?? ""} onChange={(e) => onChange(e.target.value || null)}>
      <option value="">All displays</option>
      {value && !displays.some((display) => display.id === value) && <option value={value}>Disconnected display</option>}
      {displays.map((display) => <option key={display.id} value={display.id}>{display.name} · {display.bounds.w}×{display.bounds.h}</option>)}
    </select>
  </label>;
}
