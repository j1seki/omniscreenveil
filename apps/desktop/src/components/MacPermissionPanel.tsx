import { useEffect, useState } from "react";
import * as api from "../api";
import type { MacPermission, MacPermissions } from "../types";
import { Button } from "./ui/button";

const features = [
  { id: "screen_recording", label: "Screen Recording", detail: "Required for Stream capture." },
  { id: "accessibility", label: "Accessibility", detail: "Required only for strict hide." },
] as const;

export function MacPermissionPanel({ permissions, onRefresh }: {
  permissions: MacPermissions;
  onRefresh: () => void;
}) {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    window.addEventListener("focus", onRefresh);
    return () => window.removeEventListener("focus", onRefresh);
  }, [onRefresh]);

  async function request(permission: MacPermission, settings: boolean) {
    setPending(true);
    setError(null);
    try {
      await api.macPermission(permission, settings);
      if (!settings) onRefresh();
    } catch (err) {
      setError(String(err));
    } finally {
      setPending(false);
    }
  }

  return (
    <section aria-label="macOS permissions" className="mb-4 space-y-2 rounded-lg border border-border bg-panel p-3 text-sm">
      <h2 className="font-semibold">Set up macOS permissions</h2>
      <p className="text-muted">
        Allow only the features you want to use. If macOS doesn’t show a prompt, open the permission’s settings and enable OmniScreenVeil. macOS may ask you to quit and reopen after allowing Screen Recording.
      </p>
      {features.map(({ id, label, detail }) => (
        <div key={id} className="flex flex-wrap items-center gap-2">
          <div className="flex-1"><strong>{label}</strong> · <span className="text-muted">{detail}</span></div>
          {permissions[id] ? <span className="text-success">Allowed</span> : <>
            <Button type="button" size="sm" disabled={pending} onClick={() => void request(id, false)}>Request {label}</Button>
            <Button type="button" size="sm" variant="secondary" disabled={pending} aria-label={`Open ${label} settings`} onClick={() => void request(id, true)}>Open settings</Button>
          </>}
        </div>
      ))}
      {error && <p role="alert" className="text-danger">{error}</p>}
    </section>
  );
}
