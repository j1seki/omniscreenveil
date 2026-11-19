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
  ShieldCheck,
  ShieldOff,
  Sparkles,
  Sun,
  Trash2,
  Waypoints,
} from "lucide-react";
import * as api from "./api";
import { useTheme } from "./lib/theme";
import { useUpdater } from "./lib/updater";
import type {
  AutoArm,
  AppKey,
  Capabilities,
  Inventory,
  Profile,
  ProtectionMode,
  SurfacePreview,
  TickReport,
  WindowInfo,
} from "./types";
import { findRuleForWindow } from "./types";
import { BackendBanner } from "./components/BackendBanner";
import { CommandPalette, type Command as CommandType } from "./components/CommandPalette";
import { ConfirmDialog } from "./components/ConfirmDialog";
import { MacPermissionPanel } from "./components/MacPermissionPanel";
import { Sidebar, type Page } from "./components/Sidebar";
import { MOD, TitleBar } from "./components/TitleBar";
import { Toaster, type Toast } from "./components/Toaster";
import { TooltipProvider } from "./components/ui/tooltip";
import { AuditPage, CapabilitiesPage, RulesPage, SurfacePage } from "./pages/InspectorPages";
import { OverviewPage } from "./pages/OverviewPage";
import { UpdatesPage } from "./pages/UpdatesPage";
import { WindowsPage } from "./pages/WindowsPage";
import { SettingsPage } from "./pages/SettingsPage";

export default function App() {
  const { theme, toggle: toggleTheme } = useTheme();
  const [page, setPage] = useState<Page>("overview");
  const [caps, setCaps] = useState<Capabilities | null>(null);
  const [matrix, setMatrix] = useState<Capabilities[]>([]);
  const [inv, setInv] = useState<Inventory | null>(null);
  const [profile, setProfile] = useState<Profile | null>(null);
  const [preview, setPreview] = useState<SurfacePreview | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [rendering, setRendering] = useState(false);
  const previewRequest = useRef(0);
  const previewInFlight = useRef(false);
  const [previewRevision, setPreviewRevision] = useState(0);
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

  const invalidatePreview = useCallback(() => {
    ++previewRequest.current;
    setPreview(null);
    setPreviewError(null);
    setRendering(false);
  }, []);

  const renderPreview = useCallback(async (monitor: string | null) => {
    if (previewInFlight.current || document.hidden) return;
    previewInFlight.current = true;
    const request = ++previewRequest.current;
    setPreviewError(null);
    setRendering(true);
    try {
      const frame = await api.preview(monitor);
      if (request === previewRequest.current) setPreview(frame);
    } catch (err) {
      if (request === previewRequest.current) { setPreview(null); setPreviewError(String(err)); }
    } finally {
      previewInFlight.current = false;
      if (request === previewRequest.current) setRendering(false);
    }
  }, []);

  useEffect(() => {
    if (page !== "overview") return;
    const capture = () => { if (!document.hidden) void renderPreview(null); };
    capture();
    const timer = window.setInterval(capture, 2500);
    window.addEventListener("focus", capture);
    document.addEventListener("visibilitychange", capture);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener("focus", capture);
      document.removeEventListener("visibilitychange", capture);
      invalidatePreview();
    };
  }, [page, previewRevision, invalidatePreview, renderPreview]);

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
        invalidatePreview();
        setPreviewRevision((r) => r + 1);
        void refresh("event", true);
      });
      if (cancelled) stop();
      else unlisten = stop;
    })().catch((err) => { if (!cancelled) setError(String(err)); });
    return () => { cancelled = true; unlisten?.(); };
  }, [refresh, invalidatePreview]);

  function reportProtection(report: TickReport, message: string) {
    const failure = report.failed.length
      ? `${report.failed.length} protection changes failed: ${report.failed[0][1]}`
      : null;
    setError(failure);
    notify(failure ?? message, failure ? "error" : "info");
  }

  async function setProtection(window: WindowInfo, enabled: boolean, mode: ProtectionMode) {
    setBusy(`window-${window.id}`);
    invalidatePreview();
    try {
      if (enabled) {
        await api.addAppRule(window.app, mode);
      } else {
        await api.removeAppRules(window.app);
        await api.removeWindowRules(window.id);
      }
      const report = await api.tick(false);
      await refresh("tick", true);
      reportProtection(report, `${window.app_name} ${enabled ? "rule saved" : "rule removed"}`);
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
      setConfirm({ title: "Disarm protection?", description: "Protected windows may become visible in the Surface or in a current share. Your rules are kept and automation is paused for this session until you arm again.", label: "Disarm", danger: true, action: () => { void disarmNow(); } });
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
      const failure = report.failed.length ? `Release failed: ${report.failed.join(", ")}` : null;
      setError(failure);
      notify(failure ?? "Protection off · automation paused · rules kept", failure ? "error" : "info");
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
      notify(`Auto-arm policy: ${v.replaceAll("_", " ")}`, "info");
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
    { id: "nav-surface", label: "Go to Veil Surface", group: "Navigation", icon: PanelsTopLeft, shortcut: `${MOD}3`, run: () => setPage("surface") },
    { id: "nav-audit", label: "Go to Capture test", group: "Navigation", icon: ScanSearch, shortcut: `${MOD}4`, run: () => setPage("audit") },
    { id: "nav-rules", label: "Go to Rules & automation", group: "Navigation", icon: Waypoints, shortcut: `${MOD}5`, run: () => setPage("rules") },
    { id: "nav-capabilities", label: "Go to Capabilities", group: "Navigation", icon: Settings2, shortcut: `${MOD}6`, run: () => setPage("capabilities") },
    { id: "nav-updates", label: "Go to Updates", group: "Navigation", icon: Download, shortcut: `${MOD}7`, run: () => setPage("updates") },
    { id: "nav-settings", label: "Go to Settings", group: "Navigation", icon: Settings2, shortcut: `${MOD}8`, run: () => setPage("settings") },
    { id: "arm", label: isArmed ? "Disarm protection" : "Arm protection", group: "Protection", icon: isArmed ? ShieldOff : Shield, keywords: "toggle veil shield", run: toggleArm },
    { id: "protect-all", label: "Protect all windows", group: "Protection", icon: Sparkles, keywords: "bulk select everything", run: () => { void protectAllWindows(); } },
    { id: "open-surface", label: "Open Surface window", group: "Protection", icon: PanelsTopLeft, keywords: "share screen", run: () => { void api.openSurface().catch((e) => notify(String(e), "error")); } },
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
        setPage((["overview", "windows", "surface", "audit", "rules", "capabilities", "updates", "settings"] as Page[])[Number(e.key) - 1]);
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
            onOpenSurface={() => { void api.openSurface().catch((e) => notify(String(e), "error")); }}
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
                previewError={previewError}
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
            {page === "surface" && (
              <SurfacePage
                preview={preview}
                error={previewError}
                busy={busy !== null || rendering}
                onRender={renderPreview}
                revision={previewRevision}
                platform={caps?.platform}
                onDisplayChange={invalidatePreview}
                onOpen={() => { void api.openSurface().catch((e) => notify(String(e), "error")); }}
              />
            )}
            {page === "audit" && (
              <AuditPage armed={isArmed} live={Boolean(inv?.backend.live)} />
            )}
            {page === "rules" && (
              <RulesPage
                profile={profile}
                windows={inv?.windows ?? []}
                automationPaused={Boolean(inv?.automation_paused)}
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

          <footer className="flex h-8 shrink-0 items-center justify-between border-t border-border bg-app px-6 text-[10px] text-faint">
            <span className="flex items-center gap-2">
              <kbd className="rounded border border-border bg-panel px-1.5 py-0.5 font-mono">{MOD}K</kbd>
              commands
              <span className="text-border-strong">·</span>
              <kbd className="rounded border border-border bg-panel px-1.5 py-0.5 font-mono">{MOD}D</kbd>
              theme
              <span className="text-border-strong">·</span>
              <kbd className="rounded border border-border bg-panel px-1.5 py-0.5 font-mono">{MOD}1–8</kbd>
              navigate
            </span>
            <span className="flex items-center gap-1.5">
              <ShieldCheck className="size-3 text-success" />
              No telemetry · signed GitHub updates
            </span>
          </footer>
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
