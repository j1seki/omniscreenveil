import { useCallback, useEffect, useMemo, useState } from "react";
import * as api from "./api";
import {
  LEVEL_BADGES,
  MODES,
  PLATFORM_LABELS,
  levelBadge,
  levelDetail,
  modeLabel,
  type AppliedProtection,
  type BackendInfo,
  type Capabilities,
  type Inventory,
  type Profile,
  type ProtectionMode,
  type Rule,
  type SurfacePreview,
  type VerificationRun,
  type WindowInfo,
} from "./types";

function ShieldMark({ size = 22 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true">
      <path
        d="M12 2.5 20 6v6.2c0 4.6-3.2 8.1-8 9.3-4.8-1.2-8-4.7-8-9.3V6l8-3.5Z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinejoin="round"
      />
      <path
        d="M8.4 12.2 11 14.8l4.8-5"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function normalizeApp(app: { value: string }): string {
  return app.value.replaceAll("\\", "/").toLowerCase();
}

function ruleFor(profile: Profile | null, window: WindowInfo): Rule | undefined {
  return profile?.rules.find(
    (rule) =>
      rule.enabled &&
      rule.target.kind === "app" &&
      normalizeApp(rule.target.value) === normalizeApp(window.app),
  );
}

function preferredMode(caps: Capabilities | null): ProtectionMode {
  if (!caps) return "surface";
  for (const mode of ["exclusion", "surface", "strict_hide"] as ProtectionMode[]) {
    if (caps.modes[mode].level !== "unsupported") return mode;
  }
  return "strict_hide";
}

function appliedWindow(applied: AppliedProtection[], window: WindowInfo) {
  return applied.find((entry) => entry.window === window.id);
}

function OutcomeBadge({ outcome }: { outcome: VerificationRun["outcome"] }) {
  const label = outcome === "protected" ? "Protected" : outcome === "visible" ? "Visible" : "Inconclusive";
  return <span className={`badge outcome-${outcome}`}>{label}</span>;
}

function LevelPill({ level }: { level: Capabilities["modes"][ProtectionMode] }) {
  return <span className={`badge level-${level.level}`}>{levelBadge(level)}</span>;
}

export default function App() {
  const [caps, setCaps] = useState<Capabilities | null>(null);
  const [matrix, setMatrix] = useState<Capabilities[]>([]);
  const [inv, setInv] = useState<Inventory | null>(null);
  const [profile, setProfile] = useState<Profile | null>(null);
  const [preview, setPreview] = useState<SurfacePreview | null>(null);
  const [verification, setVerification] = useState<VerificationRun | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async (label = "refresh") => {
    setBusy(label);
    try {
      const [nextInventory, nextProfile, nextPreview] = await Promise.all([
        api.inventory(),
        api.getProfile(),
        api.preview(),
      ]);
      setInv(nextInventory);
      setProfile(nextProfile);
      setPreview(nextPreview);
      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(null);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const [nextCaps, nextMatrix] = await Promise.all([
          api.capabilities(),
          api.capabilityMatrix(),
        ]);
        if (cancelled) return;
        setCaps(nextCaps);
        setMatrix(nextMatrix);
      } catch (err) {
        if (!cancelled) setError(String(err));
      }
    })();
    void refresh();
    return () => {
      cancelled = true;
    };
  }, [refresh]);

  useEffect(() => {
    if (!api.IN_TAURI) return;
    let unlisten: (() => void) | undefined;
    void (async () => {
      const { listen } = await import("@tauri-apps/api/event");
      unlisten = await listen("veil://changed", () => {
        void refresh();
      });
    })();
    return () => unlisten?.();
  }, [refresh]);

  const protectedWindows = useMemo(() => {
    if (!inv || !profile) return [];
    return inv.windows.filter((window) => ruleFor(profile, window));
  }, [inv, profile]);

  const backend: BackendInfo | null = inv?.backend ?? null;

  async function setProtection(
    window: WindowInfo,
    enabled: boolean,
    mode: ProtectionMode,
  ) {
    setBusy(`window-${window.id}`);
    try {
      if (enabled) {
        await api.addAppRule(window.app, mode);
      } else {
        await api.removeAppRules(window.app);
      }
      await api.tick();
      await refresh("tick");
      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(null);
    }
  }

  async function armNow() {
    setBusy("arm");
    try {
      await api.tick();
      await refresh("tick");
      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(null);
    }
  }

  async function releaseEverything() {
    setBusy("release");
    try {
      await api.releaseAll();
      await refresh("release");
      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(null);
    }
  }

  async function runVerification() {
    setBusy("verify");
    try {
      setVerification(await api.verify());
      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(null);
    }
  }

  async function clearRules() {
    setBusy("reset");
    try {
      await api.resetProfile();
      await api.tick();
      await refresh("reset");
      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(null);
    }
  }

  const busyMode = busy !== null;

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <span className="brand-mark">
            <ShieldMark size={24} />
          </span>
          <div>
            <h1>OmniScreenVeil</h1>
            <p className="brand-sub">Selective screen-capture privacy</p>
          </div>
        </div>
        <div className="topbar-right">
          {backend && (
            <span className={`pill ${backend.live ? "pill-live" : "pill-sim"}`}>
              <span className="dot" />
              {backend.name}
            </span>
          )}
          <span className="platform-pill">
            {caps ? PLATFORM_LABELS[caps.platform] : "Detecting…"}
          </span>
        </div>
      </header>

      {backend && (
        <div className={`banner ${backend.live ? "banner-live" : "banner-sim"}`}>
          <strong>{backend.live ? "Live backend." : "Preview / simulation."}</strong>{" "}
          {backend.note}
        </div>
      )}

      {error && (
        <div className="banner banner-error" role="alert">
          <strong>Command failed.</strong> {error}
        </div>
      )}

      <main className="layout">
        <section className="panel windows-panel">
          <div className="panel-head">
            <div>
              <h2>Windows</h2>
              <p className="subtle">
                {inv ? `${inv.windows.length} top-level windows` : "Loading…"} ·{" "}
                {protectedWindows.length} protected
              </p>
            </div>
            <button className="btn ghost" onClick={() => void refresh()} disabled={busyMode}>
              {busy === "refresh" ? "Refreshing…" : "Refresh"}
            </button>
          </div>

          <div className="window-list">
            {inv?.windows.length === 0 && (
              <p className="empty">
                No protectable windows reported. Start an application, then refresh.
              </p>
            )}
            {inv?.windows.map((window) => {
              const rule = ruleFor(profile, window);
              const enabled = Boolean(rule);
              const mode = rule?.mode ?? preferredMode(caps);
              const live = appliedWindow(inv.applied, window);
              return (
                <article className={`window-row ${enabled ? "is-protected" : ""}`} key={window.id}>
                  <div className="window-main">
                    <div className="window-title">
                      <span className="app-name">{window.app_name}</span>
                      {window.title && <span className="window-caption">{window.title}</span>}
                    </div>
                    <div className="window-meta">
                      <span className="mono">0x{window.id.toString(16)}</span>
                      <span className="mono">pid {window.pid}</span>
                      <span className="mono">
                        {window.bounds.w}×{window.bounds.h} @ {window.bounds.x},{window.bounds.y}
                      </span>
                      {enabled && live && (
                        <span className={`badge status-${live.status.status}`}>
                          {live.status.status === "applied"
                            ? modeLabel(live.mode)
                            : live.status.status}
                        </span>
                      )}
                    </div>
                  </div>
                  <div className="window-actions">
                    <select
                      className="mode-select"
                      aria-label={`Protection mode for ${window.app_name}`}
                      value={mode}
                      disabled={busyMode}
                      onChange={(event) => {
                        const next = event.target.value as ProtectionMode;
                        void setProtection(window, true, next);
                      }}
                    >
                      {MODES.map((entry) => {
                        const level = caps?.modes[entry.id];
                        const unsupported = level?.level === "unsupported";
                        return (
                          <option key={entry.id} value={entry.id} disabled={unsupported}>
                            {entry.label}
                            {unsupported ? " (unsupported)" : level?.level === "partial" ? " (partial)" : ""}
                          </option>
                        );
                      })}
                    </select>
                    <button
                      role="switch"
                      aria-checked={enabled}
                      aria-label={`Protect ${window.app_name}`}
                      className={`switch ${enabled ? "on" : ""}`}
                      disabled={busyMode}
                      onClick={() => void setProtection(window, !enabled, mode)}
                    >
                      <span className="knob" />
                    </button>
                  </div>
                </article>
              );
            })}
          </div>

          <div className="panel-foot buttons">
            <button className="btn" onClick={() => void armNow()} disabled={busyMode}>
              Arm now
            </button>
            <button className="btn" onClick={() => void releaseEverything()} disabled={busyMode}>
              Release all
            </button>
            <button className="btn ghost" onClick={() => void clearRules()} disabled={busyMode}>
              Clear rules
            </button>
          </div>
        </section>

        <aside className="sidebar">
          <section className="panel">
            <div className="panel-head">
              <div>
                <h2>Verification</h2>
                <p className="subtle">Pixels, not return codes.</p>
              </div>
              <button className="btn" onClick={() => void runVerification()} disabled={busyMode}>
                {busy === "verify" ? "Capturing…" : "Run"}
              </button>
            </div>
            {verification ? (
              <div className="verify-card">
                <div className="verify-line">
                  <OutcomeBadge outcome={verification.outcome} />
                  <span className="badge neutral">{verification.record.path}</span>
                </div>
                <p className="verify-detail">{verification.record.detail}</p>
                <dl className="kv">
                  <div>
                    <dt>Baseline</dt>
                    <dd>{verification.baseline.found ? "canary visible" : "not seen"}</dd>
                  </div>
                  <div>
                    <dt>After arming</dt>
                    <dd>{verification.after.found ? "canary visible" : "canary absent"}</dd>
                  </div>
                  <div>
                    <dt>Checked</dt>
                    <dd>{new Date(verification.record.checked_at_unix * 1000).toLocaleString()}</dd>
                  </div>
                  <div>
                    <dt>Build</dt>
                    <dd className="mono">{verification.record.os_build}</dd>
                  </div>
                </dl>
              </div>
            ) : (
              <p className="subtle">
                Runs the headless harness: composite an 8×8 canary, capture, arm, capture again.
                A missing baseline is reported as inconclusive, never as protected.
              </p>
            )}
          </section>

          <section className="panel">
            <div className="panel-head">
              <div>
                <h2>Veil Surface</h2>
                <p className="subtle">
                  {preview
                    ? `${preview.width}×${preview.height} · ${preview.masked} masked · ${preview.source}`
                    : "Rendering…"}
                </p>
              </div>
              <div className="panel-actions">
                <button
                  className="btn ghost"
                  onClick={() => void refresh("preview")}
                  disabled={busyMode}
                >
                  Render
                </button>
                <button
                  className="btn"
                  onClick={() => void api.openSurface().catch((err) => setError(String(err)))}
                  disabled={busyMode}
                >
                  Open window
                </button>
              </div>
            </div>
            <div className="preview-frame">
              {preview?.png_base64 ? (
                <img
                  src={`data:image/png;base64,${preview.png_base64}`}
                  alt="Veil Surface preview with protected windows masked"
                />
              ) : (
                <div className="preview-empty">No surface rendered yet.</div>
              )}
            </div>
            <p className="subtle">
              Share this window instead of your whole desktop. Protected windows stay usable
              locally and appear as the checker pattern to the audience.
            </p>
          </section>

          <section className="panel">
            <div className="panel-head">
              <div>
                <h2>Capabilities</h2>
                <p className="subtle">What is actually possible on this platform.</p>
              </div>
            </div>
            <div className="cap-list">
              {caps &&
                MODES.map((entry) => {
                  const level = caps.modes[entry.id];
                  return (
                    <div className="cap-row" key={entry.id}>
                      <div className="cap-name">
                        <strong>{entry.label}</strong>
                        <LevelPill level={level} />
                      </div>
                      <p className="subtle">{levelDetail(level)}</p>
                    </div>
                  );
                })}
            </div>

            {matrix.length > 0 && (
              <details className="matrix">
                <summary>Full support matrix</summary>
                <table>
                  <thead>
                    <tr>
                      <th>Mode</th>
                      {matrix.map((entry) => (
                        <th key={entry.platform}>{PLATFORM_LABELS[entry.platform]}</th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {MODES.map((entry) => (
                      <tr key={entry.id}>
                        <td>{entry.label}</td>
                        {matrix.map((platformCaps) => {
                          const level = platformCaps.modes[entry.id];
                          return (
                            <td
                              key={platformCaps.platform}
                              className={`matrix-cell level-${level.level}`}
                              title={levelDetail(level)}
                            >
                              {LEVEL_BADGES[level.level]}
                            </td>
                          );
                        })}
                      </tr>
                    ))}
                  </tbody>
                </table>
                <p className="subtle">
                  ✓ native · ◐ partial (documented constraint) · — unsupported. Exclusion is
                  never claimed on macOS, X11 or Wayland.
                </p>
              </details>
            )}
          </section>
        </aside>
      </main>

      <footer className="footer">
        <span>
          <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>V</kbd> arm / release · tray menu mirrors
          the same actions
        </span>
        <span className="subtle">
          No telemetry. No network. Everything runs locally.
        </span>
      </footer>
    </div>
  );
}
