import { useCallback, useEffect, useMemo, useState } from "react";
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
  Capabilities,
  Inventory,
  Profile,
  ProtectionMode,
  SurfacePreview,
  VerificationRun,
  WindowInfo,
} from "./types";
import { findRuleForWindow } from "./types";
import { BackendBanner } from "./components/BackendBanner";
import { CommandPalette, type Command as CommandType } from "./components/CommandPalette";
import { ConfirmDialog } from "./components/ConfirmDialog";
import { Sidebar, type Page } from "./components/Sidebar";
import { MOD, TitleBar } from "./components/TitleBar";
import { Toaster, type Toast } from "./components/Toaster";
import { TooltipProvider } from "./components/ui/tooltip";
import { AuditPage, CapabilitiesPage, RulesPage, SurfacePage } from "./pages/InspectorPages";
import { OverviewPage } from "./pages/OverviewPage";
import { UpdatesPage } from "./pages/UpdatesPage";
import { WindowsPage } from "./pages/WindowsPage";

export default function App() {
  const { theme, toggle: toggleTheme } = useTheme();
  const [page, setPage] = useState<Page>("overview");
  const [caps, setCaps] = useState<Capabilities | null>(null);
  const [matrix, setMatrix] = useState<Capabilities[]>([]);
  const [inv, setInv] = useState<Inventory | null>(null);
  const [profile, setProfile] = useState<Profile | null>(null);
  const [preview, setPreview] = useState<SurfacePreview | null>(null);
  const [verification, setVerification] = useState<VerificationRun | null>(null);
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
    setBusy(label);
    try {
      const [nextInventory, nextProfile, nextPreview] = await Promise.all([
        api.inventory(),
        api.getProfile(),
        api.preview().then((value) => ({ value, error: null as string | null })).catch((err) => ({ value: null, error: String(err) })),
      ]);
      setInv(nextInventory);
      setProfile(nextProfile);
      setPreview(nextPreview.value);
      setError(nextPreview.error);
      if (!quiet && nextPreview.error) notify(nextPreview.error, "error");
    } catch (err) {
      setError(String(err));
      if (!quiet) notify(String(err), "error");
    } finally {
      setBusy(null);
    }
  }, [notify]);

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
    let unlisten: (() => void) | undefined;
    void (async () => {
      const { listen } = await import("@tauri-apps/api/event");
      unlisten = await listen("veil://changed", () => { void refresh("event", true); });
    })();
    return () => unlisten?.();
  }, [refresh]);

  async function setProtection(window: WindowInfo, enabled: boolean, mode: ProtectionMode) {
    setBusy(`window-${window.id}`);
    try {
      if (enabled) {
        await api.addAppRule(window.app, mode);
        notify(`${window.app_name} protected with ${mode.replace("_", " ")}`);
      } else {
        await api.removeAppRules(window.app);
        await api.removeWindowRules(window.id);
        notify(`${window.app_name} rule removed`, "info");
      }
      await api.tick();
      await refresh("tick", true);
      setError(null);
    } catch (err) {
      setError(String(err));
      notify(String(err), "error");
    } finally {
      setBusy(null);
    }
  }

  async function armNow() {
    setBusy("arm");
    try {
      const report = await api.tick();
      await refresh("tick", true);
      setError(null);
      notify(`Protection armed · ${report.applied.length} applied`);
    } catch (err) {
      setError(String(err));
      notify(String(err), "error");
    } finally {
      setBusy(null);
    }
  }

  async function protectAllWindows() {
    setBusy("protect-all");
    try {
      const mode: ProtectionMode = caps?.modes.exclusion.level !== "unsupported" ? "exclusion" : "surface";
      await api.protectAll(mode);
      await refresh("protect-all", true);
      setError(null);
      notify("All protectable windows covered");
    } catch (err) {
      setError(String(err));
      notify(String(err), "error");
    } finally {
      setBusy(null);
    }
  }

  async function releaseEverything() {
    setBusy("release");
    try {
      const report = await api.releaseAll();
      await refresh("release", true);
      setError(null);
      notify(report.failed.length ? `Release failed: ${report.failed.join("; ")}` : `Released ${report.released} windows · rules preserved`,
        report.failed.length ? "error" : "info");
    } catch (err) {
      setError(String(err));
      notify(String(err), "error");
    } finally {
      setBusy(null);
    }
  }

  async function runVerification() {
    setBusy("verify");
    try {
      const res = await api.verify();
      setVerification(res);
      setError(null);
      notify(`Synthetic audit: ${res.outcome}`, res.outcome === "protected" ? "success" : "info");
    } catch (err) {
      setError(String(err));
      notify(String(err), "error");
    } finally {
      setBusy(null);
    }
  }

  async function clearRules() {
    setBusy("reset");
    try {
      await api.resetProfile();
      await api.releaseAll();
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

  const windows = inv?.windows ?? [];
  const protectedCount = windows.filter((w) => Boolean(findRuleForWindow(profile, w))).length;
  const isArmed = Boolean(inv?.armed) || (inv?.applied.length ?? 0) > 0;
  const emptyInventory: Inventory = {
    windows: [],
    backend: { platform: caps?.platform ?? "linux_x11", name: "", live: false, note: "" },
    applied: [],
  };

  const commands = useMemo<CommandType[]>(() => [
    { id: "nav-overview", label: "Go to Overview", group: "Navigation", icon: LayoutDashboard, shortcut: `${MOD}1`, run: () => setPage("overview") },
    { id: "nav-windows", label: "Go to Windows", group: "Navigation", icon: ListFilter, shortcut: `${MOD}2`, run: () => setPage("windows") },
    { id: "nav-surface", label: "Go to Veil Surface", group: "Navigation", icon: PanelsTopLeft, shortcut: `${MOD}3`, run: () => setPage("surface") },
    { id: "nav-audit", label: "Go to Canary audit", group: "Navigation", icon: ScanSearch, shortcut: `${MOD}4`, run: () => setPage("audit") },
    { id: "nav-rules", label: "Go to Rules & automation", group: "Navigation", icon: Waypoints, shortcut: `${MOD}5`, run: () => setPage("rules") },
    { id: "nav-capabilities", label: "Go to Capabilities", group: "Navigation", icon: Settings2, shortcut: `${MOD}6`, run: () => setPage("capabilities") },
    { id: "nav-updates", label: "Go to Updates", group: "Navigation", icon: Download, shortcut: `${MOD}7`, run: () => setPage("updates") },
    { id: "arm", label: isArmed ? "Disarm protection" : "Arm protection", group: "Protection", icon: isArmed ? ShieldOff : Shield, keywords: "toggle veil shield", run: () => { void (isArmed ? releaseEverything() : protectedCount ? armNow() : protectAllWindows()); } },
    { id: "protect-all", label: "Protect all windows", group: "Protection", icon: Sparkles, keywords: "bulk select everything", run: () => { void protectAllWindows(); } },
    { id: "open-surface", label: "Open Surface window", group: "Protection", icon: PanelsTopLeft, keywords: "share screen", run: () => { void api.openSurface().catch((e) => notify(String(e), "error")); } },
    { id: "audit", label: "Run canary audit", group: "Verification", icon: ScanSearch, keywords: "synthetic test pixel", run: () => { setPage("audit"); void runVerification(); } },
    { id: "refresh", label: "Refresh inventory", group: "Verification", icon: RefreshCw, shortcut: `${MOD}R`, run: () => { void refresh(); } },
    { id: "theme", label: `Switch to ${theme === "dark" ? "light" : "dark"} theme`, group: "Preferences", icon: theme === "dark" ? Sun : Moon, shortcut: `${MOD}D`, run: toggleTheme },
    { id: "updates", label: "Check for updates", group: "Preferences", icon: Download, keywords: "upgrade release stable beta", run: () => { setPage("updates"); void updater.check("manual"); } },
    { id: "shortcuts", label: "Show keyboard shortcuts", group: "Preferences", icon: CircleHelp, keywords: "help keys", run: () => setPaletteOpen(true) },
    { id: "clear", label: "Purge all rules", group: "Danger zone", icon: Trash2, danger: true, keywords: "reset clear delete", run: () => setConfirm({ title: "Purge all rules?", description: "This removes every persistent rule and releases all protection. This cannot be undone.", label: "Purge rules", danger: true, action: () => { void clearRules(); } }) },
  // eslint-disable-next-line react-hooks/exhaustive-deps
  ], [isArmed, protectedCount, theme, refresh, toggleTheme, notify, updater.check]);

  const openPalette = useCallback(() => setPaletteOpen(true), []);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.key.toLowerCase() === "k") { e.preventDefault(); setPaletteOpen((o) => !o); return; }
      if (mod && e.key.toLowerCase() === "d") { e.preventDefault(); toggleTheme(); return; }
      if (mod && e.key.toLowerCase() === "r") { e.preventDefault(); void refresh(); return; }
      if (mod && /^[1-7]$/.test(e.key)) {
        e.preventDefault();
        setPage((["overview", "windows", "surface", "audit", "rules", "capabilities", "updates"] as Page[])[Number(e.key) - 1]);
        return;
      }
      if (e.key === "Escape") { setPaletteOpen(false); setConfirm(null); }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [refresh, toggleTheme]);

  function toggleArm() {
    if (isArmed) void releaseEverything();
    else if (protectedCount) void armNow();
    else void protectAllWindows();
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
          protectedCount={protectedCount}
          version={updater.currentVersion}
          updateAvailable={Boolean(updater.update)}
          onToggleArm={toggleArm}
          onOpenSurface={() => { void api.openSurface().catch((e) => notify(String(e), "error")); }}
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
            {page === "overview" && (
              <OverviewPage
                inventory={inv ?? emptyInventory}
                profile={profile}
                caps={caps}
                preview={preview}
                backend={inv?.backend ?? null}
                onNavigate={(p) => setPage(p)}
                onToggleWindow={(w) => {
                  const rule = findRuleForWindow(profile, w);
                  const mode = rule?.mode ?? (caps?.modes.exclusion.level !== "unsupported" ? "exclusion" : "surface");
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
                  const mode = rule?.mode ?? (caps?.modes.exclusion.level !== "unsupported" ? "exclusion" : "surface");
                  void setProtection(w, !rule, mode);
                }}
                onMode={(w, mode) => void setProtection(w, true, mode)}
              />
            )}
            {page === "surface" && (
              <SurfacePage
                preview={preview}
                busy={busy !== null}
                onRender={() => void refresh("preview")}
                onOpen={() => { void api.openSurface().catch((e) => notify(String(e), "error")); }}
              />
            )}
            {page === "audit" && (
              <AuditPage verification={verification} busy={busy !== null} onRun={() => void runVerification()} />
            )}
            {page === "rules" && (
              <RulesPage
                profile={profile}
                busy={busy !== null}
                onAutoArm={(v) => void setAutoArm(v)}
                onToggleRule={async (id, enabled) => {
                  setBusy("rules");
                  try {
                    await api.setRuleEnabled(id, enabled);
                    await api.tick();
                    await refresh("rules", true);
                    setError(null);
                  } catch (err) { setError(String(err)); notify(String(err), "error"); }
                  finally { setBusy(null); }
                }}
                onRemoveRule={async (id) => {
                  setBusy("rules");
                  try {
                    await api.removeRule(id);
                    await api.tick();
                    await refresh("rules", true);
                    setError(null);
                  } catch (err) { setError(String(err)); notify(String(err), "error"); }
                  finally { setBusy(null); }
                }}
                onClear={() => setConfirm({ title: "Purge all rules?", description: "Every persistent rule will be removed and all protection released.", label: "Purge rules", danger: true, action: () => { void clearRules(); } })}
              />
            )}
            {page === "capabilities" && <CapabilitiesPage caps={caps} matrix={matrix} />}
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
              <kbd className="rounded border border-border bg-panel px-1.5 py-0.5 font-mono">{MOD}1–7</kbd>
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
