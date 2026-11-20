import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  CircleHelp,
  Download,
  LayoutDashboard,
  ListFilter,
  Moon,
  PanelsTopLeft,
  RefreshCw,
  ScanSearch,
  Settings2,
  Shield,
  ShieldOff,
  Sparkles,
  Sun,
  Trash2,
  Waypoints,
} from "lucide-react";
import * as api from "./api";
import { useCapturePlayback } from "./lib/capturePlayback";
import { useCaptureSettings } from "./lib/captureSettings";
import { useCaptureFeedback } from "./lib/captureFeedback";
import { useTheme } from "./lib/theme";
import { useUpdater } from "./lib/updater";
import type {
  AutoArm,
  AppKey,
  Capabilities,
  Inventory,
  Profile,
  ProtectionMode,
  TickReport,
  WindowInfo,
} from "./types";
import { findRuleForWindow } from "./types";
import { BackendBanner } from "./components/BackendBanner";
import { CommandPalette, type Command as CommandType } from "./components/CommandPalette";
import { ConfirmDialog } from "./components/ConfirmDialog";
import { DiagnosticsBar } from "./components/DiagnosticsBar";
import { MacPermissionPanel } from "./components/MacPermissionPanel";
import { Sidebar, type Page } from "./components/Sidebar";
import { MOD, TitleBar } from "./components/TitleBar";
import { Toaster, type Toast } from "./components/Toaster";
import { TooltipProvider } from "./components/ui/tooltip";
import { CapabilitiesPage, RulesPage } from "./pages/InspectorPages";
import { OverviewPage } from "./pages/OverviewPage";
import { UpdatesPage } from "./pages/UpdatesPage";
import { WindowsPage } from "./pages/WindowsPage";
import { VerifyPage } from "./pages/VerifyPage";
import { SettingsPage } from "./pages/SettingsPage";
import { PreviewPage } from "./pages/PreviewPage";

export default function App() {
  const { theme, toggle: toggleTheme } = useTheme();
  const [page, setPage] = useState<Page>("overview");
  const [caps, setCaps] = useState<Capabilities | null>(null);
  useCaptureFeedback(caps?.platform === "windows");
  const [matrix, setMatrix] = useState<Capabilities[]>([]);
  const [inv, setInv] = useState<Inventory | null>(null);
  const [profile, setProfile] = useState<Profile | null>(null);
  const preferences = useCaptureSettings();
  const previewSettings = preferences.settings?.preview;
  const previewCapture = useCapturePlayback({
    enabled: Boolean(previewSettings) && (page === "overview" || page === "preview"),
    display: previewSettings?.display || null,
    edge: previewSettings?.max_edge ?? 960,
    interval: previewSettings?.playback === "automatic" ? previewSettings.interval_ms : undefined,
  });
  const invalidatePreview = previewCapture.invalidate;
  const preview = {
    frame: previewCapture.frame,
    message: preferences.error ?? previewCapture.message,
    capture: previewCapture.capture,
    manual: previewSettings?.playback === "manual",
    ready: Boolean(previewSettings) && !preferences.saving,
  };
  const [streamEnabled, setStreamEnabled] = useState(false);
  const [streamBusy, setStreamBusy] = useState(false);
  const refreshRequest = useRef(0);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [confirm, setConfirm] = useState<{
    title: string;
    description: string;
    label: string;
    danger?: boolean;
    action: () => void;
  } | null>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const toastId = useState(() => ({ n: 0 }))[0];

  const notify = useCallback((message: string, kind: Toast["kind"] = "success") => {
    if (kind === "error") api.recordLog("error", message);
    const id = ++toastId.n;
    setToasts((t) => [...t, { id, message, kind }]);
    window.setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), 3600);
  }, [toastId]);

  const updater = useUpdater(notify);

  const refresh = useCallback(async (label = "refresh", quiet = false) => {
    const request = ++refreshRequest.current;
    if (!quiet) setError(null);
    try {
      const [nextInventory, nextProfile] = await Promise.all([api.inventory(), api.getProfile()]);
      if (request !== refreshRequest.current) return;
      setInv(nextInventory);
      setProfile(nextProfile);
    } catch (err) {
      if (request === refreshRequest.current) {
        if (!quiet || label === "initial") setError(String(err));
        if (!quiet) notify(`${label}: ${String(err)}`, "error");
      }
    }
  }, [notify]);

  useEffect(() => {
    if (!api.IN_TAURI) return;
    let mounted = true;
    let unsubscribe: (() => void) | undefined;
    const refreshStream = () => {
      void api.streamEnabled().then((enabled) => {
        if (mounted) setStreamEnabled(enabled);
      }).catch((err) => { if (mounted) notify(String(err), "error"); });
    };
    void listen("veil://stream-status", refreshStream).then((stop) => {
      if (!mounted) { stop(); return; }
      unsubscribe = stop;
      refreshStream();
    }).catch((err) => { if (mounted) notify(String(err), "error"); });
    return () => { mounted = false; unsubscribe?.(); };
  }, [notify]);

  async function toggleStream(enabled: boolean) {
    setStreamBusy(true);
    try {
      const status = await api.setStreamEnabled(enabled);
      setStreamEnabled(status.enabled);
      if (status.enabled && status.protection) {
        const { armed, surface_rules: rules } = status.protection;
        const surface = rules ? `${rules} enabled Surface ${rules === 1 ? "rule" : "rules"}` : "no enabled Surface rules";
        notify(`Stream enabled · protection ${armed ? "armed" : "disarmed"} · ${surface}`, "info");
      }
    } catch (err) {
      notify(String(err), "error");
    } finally {
      setStreamBusy(false);
    }
  }

  function viewStream() {
    void api.openSurface().catch((err) => notify(String(err), "error"));
  }

  useEffect(() => {
    let pending = false;
    let queued = false;
    let alive = true;
    let stop: (() => void) | undefined;
    let timer: number | undefined;
    const scan = async () => {
      if (!alive || document.hidden) return;
      if (pending) { queued = true; return; }
      pending = true;
      try { await refresh("automatic", true); }
      finally {
        pending = false;
        if (queued) { queued = false; void scan(); }
      }
    };
    if (!api.IN_TAURI) return;
    void (async () => {
      let native = false;
      try {
        native = await api.nativeWindowEvents();
        const unlisten = await listen("veil://windows", () => void scan());
        if (!alive) { unlisten(); return; }
        stop = unlisten;
      } catch { native = false; }
      if (alive) timer = window.setInterval(() => { if (document.hasFocus()) void scan(); }, native ? 15000 : 2000);
    })();
    const focused = () => void scan();
    window.addEventListener("focus", focused);
    document.addEventListener("visibilitychange", focused);
    return () => { alive = false; stop?.(); window.clearInterval(timer); window.removeEventListener("focus", focused); document.removeEventListener("visibilitychange", focused); };
  }, [refresh]);

  const refreshPermissions = useCallback(() => { void refresh("permissions", true); }, [refresh]);

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
    void refresh("initial", true);
    return () => { cancelled = true; };
  }, [refresh]);

  useEffect(() => {
    if (!api.IN_TAURI) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void (async () => {
      const stop = await listen("veil://changed", () => {
        void refresh("event", true);
      });
      if (cancelled) stop();
      else unlisten = stop;
    })().catch((err) => { if (!cancelled) setError(String(err)); });
    return () => { cancelled = true; unlisten?.(); };
  }, [refresh]);

  function reportProtection(report: TickReport, message: string) {
    const failure = report.failed.length
      ? `${report.failed.length} protection changes failed: ${report.failed[0][1]}`
      : null;
    setError(failure);
    notify(failure ?? message, failure ? "error" : "info");
  }

  async function setProtection(window: WindowInfo, enabled: boolean, mode: ProtectionMode) {
    const ownWindow = inv?.own_window_ids?.includes(window.id);
    if (ownWindow && findRuleForWindow(profile, window)?.target.kind === "app") {
      notify("Edit this app-wide rule in Rules & automation.", "info");
      return;
    }
    setBusy(`window-${window.id}`);
    invalidatePreview();
    try {
      if (enabled) {
        if (ownWindow) await api.addWindowRule(window.id, mode);
        else await api.addAppRule(window.app, mode);
      } else {
        if (!ownWindow) await api.removeAppRules(window.app);
        await api.removeWindowRules(window.id);
      }
      const report = await api.tick(false);
      await refresh("tick", true);
      reportProtection(report, `${ownWindow ? window.title : window.app_name} ${enabled ? "rule saved" : "rule removed"}`);
    } catch (err) {
      setError(String(err));
      notify(String(err), "error");
    } finally {
      setBusy(null);
    }
  }

  async function armNow() {
    setBusy("arm");
    invalidatePreview();
    try {
      const report = await api.tick();
      await refresh("tick", true);
      reportProtection(report, `Protection armed · ${report.applied.length + report.unchanged} active`);
    } catch (err) {
      setError(String(err));
      notify(String(err), "error");
    } finally {
      setBusy(null);
    }
  }

  async function protectAllWindows() {
    setBusy("protect-all");
    invalidatePreview();
    try {
      const report = await api.protectAll(defaultMode);
      await refresh("protect-all", true);
      reportProtection(report, `Protection armed · ${report.applied.length + report.unchanged} active`);
    } catch (err) {
      setError(String(err));
      notify(String(err), "error");
    } finally {
      setBusy(null);
    }
  }

  async function releaseEverything() {
    if (busy) return;
    if (api.IN_TAURI) {
      setConfirm({ title: "Disarm protection?", description: "Protected windows may become visible in the Stream or in a current share. Your rules are kept. Arm protection or select an auto-arm policy to resume.", label: "Disarm", danger: true, action: () => { void disarmNow(); } });
    } else {
      await disarmNow();
    }
  }

  async function disarmNow() {
    setBusy("release");
    invalidatePreview();
    try {
      const report = await api.releaseAll();
      await refresh("release", true);
      const failure = report.remaining || report.failed.length
        ? `Disarm incomplete · ${report.remaining} windows still tracked. ${report.failed.join(", ")}`
        : null;
      setError(failure);
      notify(failure ?? `Protection off · ${report.released} windows released${report.automation_paused ? " · automation paused" : ""}`, failure ? "error" : "info");
    } catch (err) {
      setError(String(err));
      notify(String(err), "error");
    } finally {
      setBusy(null);
    }
  }

  async function clearRules() {
    setBusy("reset");
    invalidatePreview();
    try {
      await api.resetProfile();
      const report = await api.releaseAll();
      if (report.failed.length) throw new Error(`Rules cleared, but release failed: ${report.failed.join("; ")}`);
      await refresh("reset", true);
      setError(null);
      notify("All rules purged and windows restored", "info");
    } catch (err) {
      setError(String(err));
      notify(String(err), "error");
    } finally {
      setBusy(null);
    }
  }

  async function setAutoArm(v: AutoArm) {
    setBusy("auto-arm");
    invalidatePreview();
    try {
      await api.setAutoArm(v);
      await refresh("auto-arm", true);
      setError(null);
      notify(v === "off" ? "Manual protection selected" : "Auto-arm enabled", "info");
    } catch (err) {
      setError(String(err));
      notify(String(err), "error");
    } finally {
      setBusy(null);
    }
  }

  const defaultMode: ProtectionMode = caps?.modes.exclusion && caps.modes.exclusion.level !== "unsupported" ? "exclusion" : "surface";
  const appliedCount = inv?.applied.filter((record) => record.status.status === "applied").length ?? 0;
  const isArmed = Boolean(inv?.armed);
  const emptyInventory: Inventory = {
    windows: [],
    backend: { platform: caps?.platform ?? "linux_x11", name: "", live: false, note: "" },
    applied: [],
  };

  const commands: CommandType[] = [
    { id: "nav-overview", label: "Go to Overview", group: "Navigation", icon: LayoutDashboard, shortcut: `${MOD}1`, run: () => setPage("overview") },
    { id: "nav-windows", label: "Go to Windows", group: "Navigation", icon: ListFilter, shortcut: `${MOD}2`, run: () => setPage("windows") },
    { id: "nav-preview", label: "Go to Preview", group: "Navigation", icon: PanelsTopLeft, shortcut: `${MOD}3`, run: () => setPage("preview") },
    { id: "nav-audit", label: "Go to Test suite", group: "Navigation", icon: ScanSearch, shortcut: `${MOD}4`, run: () => setPage("audit") },
    { id: "nav-rules", label: "Go to Rules & automation", group: "Navigation", icon: Waypoints, shortcut: `${MOD}5`, run: () => setPage("rules") },
    { id: "nav-capabilities", label: "Go to Capabilities", group: "Navigation", icon: Settings2, shortcut: `${MOD}6`, run: () => setPage("capabilities") },
    { id: "nav-updates", label: "Go to Updates", group: "Navigation", icon: Download, shortcut: `${MOD}7`, run: () => setPage("updates") },
    { id: "nav-settings", label: "Go to Settings", group: "Navigation", icon: Settings2, shortcut: `${MOD}8`, run: () => setPage("settings") },
    { id: "arm", label: isArmed ? "Disarm protection" : "Arm protection", group: "Protection", icon: isArmed ? ShieldOff : Shield, keywords: "toggle veil shield", run: toggleArm },
    { id: "protect-all", label: "Protect all windows", group: "Protection", icon: Sparkles, keywords: "bulk select everything", run: () => { void protectAllWindows(); } },
    ...(streamEnabled ? [{ id: "open-stream", label: "Open Stream viewer", group: "Protection", icon: PanelsTopLeft, keywords: "share screen", run: viewStream }] : []),
    { id: "refresh", label: "Refresh windows", group: "Verification", icon: RefreshCw, shortcut: `${MOD}R`, run: () => { void refresh(); } },
    { id: "theme", label: `Switch to ${theme === "dark" ? "light" : "dark"} theme`, group: "Preferences", icon: theme === "dark" ? Sun : Moon, shortcut: `${MOD}D`, run: toggleTheme },
    { id: "updates", label: "Check for updates", group: "Preferences", icon: Download, keywords: "upgrade release stable beta", run: () => { setPage("updates"); void updater.check("manual"); } },
    { id: "shortcuts", label: "Show keyboard shortcuts", group: "Preferences", icon: CircleHelp, keywords: "help keys", run: () => setPaletteOpen(true) },
    { id: "clear", label: "Purge all rules", group: "Danger zone", icon: Trash2, danger: true, keywords: "reset clear delete", run: () => setConfirm({ title: "Purge all rules?", description: "This removes every persistent rule and releases all protection. This cannot be undone.", label: "Purge rules", danger: true, action: () => { void clearRules(); } }) },
  ];

  const openPalette = useCallback(() => setPaletteOpen(true), []);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.key.toLowerCase() === "k") { e.preventDefault(); setPaletteOpen((o) => !o); return; }
      if (mod && e.key.toLowerCase() === "d") { e.preventDefault(); toggleTheme(); return; }
      if (mod && e.key.toLowerCase() === "r") { e.preventDefault(); void refresh(); return; }
      if (mod && /^[1-8]$/.test(e.key)) {
        e.preventDefault();
        setPage((["overview", "windows", "preview", "audit", "rules", "capabilities", "updates", "settings"] as Page[])[Number(e.key) - 1]);
        return;
      }
      if (e.key === "Escape") { setPaletteOpen(false); setConfirm(null); }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [refresh, toggleTheme]);

  function toggleArm() {
    if (busy || !profile) return;
    if (isArmed) void releaseEverything();
    else void armNow();
  }

  return (
    <TooltipProvider>
      <div className="flex h-full w-full overflow-hidden bg-app text-text">
        <Sidebar
          page={page}
          onNavigate={setPage}
          theme={theme}
          onToggleTheme={toggleTheme}
          onOpenPalette={openPalette}
          backend={inv?.backend ?? null}
          armed={isArmed}
          protectedCount={appliedCount}
          version={updater.currentVersion}
          updateAvailable={Boolean(updater.update)}
          onToggleArm={toggleArm}
        />

        <div className="flex min-w-0 flex-1 flex-col">
          <TitleBar
            page={page}
            backend={inv?.backend ?? null}
            platform={caps?.platform ?? null}
            armed={isArmed}
            busy={busy !== null}
            onOpenPalette={openPalette}
            onRefresh={() => void refresh()}
            onToggleArm={toggleArm}
            streamEnabled={streamEnabled}
            streamControlsDisabled={streamBusy || (!streamEnabled && !inv?.backend.live)}
            onToggleStream={(enabled) => void toggleStream(enabled)}
            onOpenStream={viewStream}
          />

          <div className="px-6 pt-3">
            {inv?.backend && (
              <BackendBanner
                tone={inv.backend.live ? "info" : "warning"}
                title={inv.backend.live ? `${inv.backend.name} backend` : "Preview mode"}
              >
                {inv.backend.note}
              </BackendBanner>
            )}
            {inv?.profile_error && <div className="mt-2"><BackendBanner tone="error" title="Profile unavailable">{inv.profile_error}</BackendBanner></div>}
            {error && <div className="mt-2"><BackendBanner tone="error" title="Command failed">{error}</BackendBanner></div>}
            {updater.update && page !== "updates" && (
              <div className="mt-2 flex items-center gap-2.5 rounded-lg border border-accent/25 bg-accent-soft px-3 py-2 text-[11px] leading-snug text-muted">
                <Download className="size-3.5 shrink-0 text-accent" />
                <span className="min-w-0 flex-1 truncate">
                  <strong className="font-semibold text-accent">
                    OmniScreenVeil {updater.update.version}
                  </strong>{" "}
                  is available on the {updater.update.channel} channel.
                </span>
                <button
                  type="button"
                  onClick={() => setPage("updates")}
                  className="shrink-0 rounded-md border border-accent/30 px-2 py-1 text-[11px] font-medium text-accent transition-colors hover:bg-accent/10"
                >
                  Review update
                </button>
              </div>
            )}
          </div>

          <main className="min-h-0 flex-1 overflow-y-auto px-6 py-4">
            {inv?.mac_permissions && (!inv.mac_permissions.screen_recording || !inv.mac_permissions.accessibility) && (
              <MacPermissionPanel permissions={inv.mac_permissions} onRefresh={refreshPermissions} />
            )}
            {page === "overview" && (
              <OverviewPage
                inventory={inv ?? emptyInventory}
                profile={profile}
                caps={caps}
                preview={preview}
                busy={busy !== null}
                backend={inv?.backend ?? null}
                onNavigate={(p) => setPage(p)}
                onToggleWindow={(w) => {
                  const rule = findRuleForWindow(profile, w);
                  const mode = rule?.mode ?? defaultMode;
                  void setProtection(w, !rule, mode);
                }}
              />
            )}
            {page === "windows" && (
              <WindowsPage
                inventory={inv ?? emptyInventory}
                profile={profile}
                caps={caps}
                busy={busy !== null}
                onRefresh={() => void refresh()}
                onToggle={(w) => {
                  const rule = findRuleForWindow(profile, w);
                  const mode = rule?.mode ?? defaultMode;
                  void setProtection(w, !rule, mode);
                }}
                onMode={(w, mode) => void setProtection(w, true, mode)}
              />
            )}
            {page === "preview" && (
              <PreviewPage
                preferences={preferences}
                preview={preview}
                nativeAvailable={caps?.platform === "windows" && Boolean(inv?.backend.live)}
              />
            )}
            {page === "audit" && (
              <VerifyPage armed={isArmed} live={Boolean(inv?.backend.live)} />
            )}
            {page === "rules" && (
              <RulesPage
                profile={profile}
                windows={inv?.windows ?? []}
                automationPaused={Boolean(inv?.automation_paused)}
                captureActive={Boolean(inv?.capture_active)}
                caps={caps}
                onAdd={async (app: AppKey, mode: ProtectionMode, enabled: boolean) => {
                  setBusy("rules");
                  invalidatePreview();
                  try {
                    await api.addAppRule(app, mode, enabled);
                    const report = await api.tick(false);
                    await refresh("rules", true);
                    reportProtection(report, "Rule saved");
                  } catch (err) { setError(String(err)); notify(String(err), "error"); }
                  finally { setBusy(null); }
                }}
                busy={busy !== null}
                onAutoArm={(v) => void setAutoArm(v)}
                onToggleRule={async (id, enabled) => {
                  setBusy("rules");
                  invalidatePreview();
                  try {
                    await api.setRuleEnabled(id, enabled);
                    const report = await api.tick(false);
                    await refresh("rules", true);
                    reportProtection(report, "Rule updated");
                  } catch (err) { setError(String(err)); notify(String(err), "error"); }
                  finally { setBusy(null); }
                }}
                onRemoveRule={async (id) => {
                  setBusy("rules");
                  invalidatePreview();
                  try {
                    await api.removeRule(id);
                    const report = await api.tick(false);
                    await refresh("rules", true);
                    reportProtection(report, "Rule removed");
                  } catch (err) { setError(String(err)); notify(String(err), "error"); }
                  finally { setBusy(null); }
                }}
                onClear={() => setConfirm({ title: "Purge all rules?", description: "Every persistent rule will be removed and all protection released.", label: "Purge rules", danger: true, action: () => { void clearRules(); } })}
              />
            )}
            {page === "capabilities" && <CapabilitiesPage caps={caps} matrix={matrix} />}
            {page === "settings" && <SettingsPage theme={theme} onToggleTheme={toggleTheme} />}
            {page === "updates" && (
              <UpdatesPage
                supported={updater.supported}
                currentVersion={updater.currentVersion}
                channel={updater.channel}
                setChannel={updater.setChannel}
                autoCheck={updater.autoCheck}
                setAutoCheck={updater.setAutoCheck}
                lastChecked={updater.lastChecked}
                update={updater.update}
                manifestFound={updater.manifestFound}
                checking={updater.checking}
                installing={updater.installing}
                progress={updater.progress}
                installedVersion={updater.installedVersion}
                error={updater.error}
                check={() => { void updater.check("manual"); }}
                install={() => { void updater.install(); }}
                restart={() => { void updater.restart(); }}
              />
            )}
          </main>

          <DiagnosticsBar notify={notify} />
        </div>

        <CommandPalette open={paletteOpen} onClose={() => setPaletteOpen(false)} commands={commands} />
        <ConfirmDialog
          open={confirm !== null}
          title={confirm?.title ?? ""}
          description={confirm?.description ?? ""}
          confirmLabel={confirm?.label}
          danger={confirm?.danger}
          onCancel={() => setConfirm(null)}
          onConfirm={() => { confirm?.action(); setConfirm(null); }}
        />
        <Toaster toasts={toasts} onDismiss={(id) => setToasts((t) => t.filter((x) => x.id !== id))} />
      </div>
    </TooltipProvider>
  );
}
