import { useCallback, useEffect, useState } from "react";
import {
  Activity,
  Keyboard,
  Shield,
  ShieldCheck,
  ShieldOff,
  Sparkles,
  Terminal,
  Trash2,
} from "lucide-react";
import * as api from "./api";
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
import { findRuleForWindow, isCaptureWindow, modeLabel } from "./types";
import { cn } from "./lib/utils";
import { BackendBanner } from "./components/BackendBanner";
import { CapabilitiesCard } from "./components/CapabilitiesCard";
import { RulesCard } from "./components/RulesCard";
import { SurfaceCard } from "./components/SurfaceCard";
import { TopBar } from "./components/TopBar";
import { VerificationCard } from "./components/VerificationCard";
import { WindowList } from "./components/WindowList";
import { Button } from "./components/ui/button";
import { TooltipProvider } from "./components/ui/tooltip";

type RightTab = "all" | "surface" | "verify" | "policy";

export default function App() {
  const [caps, setCaps] = useState<Capabilities | null>(null);
  const [matrix, setMatrix] = useState<Capabilities[]>([]);
  const [inv, setInv] = useState<Inventory | null>(null);
  const [profile, setProfile] = useState<Profile | null>(null);
  const [preview, setPreview] = useState<SurfacePreview | null>(null);
  const [verification, setVerification] = useState<VerificationRun | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [rightTab, setRightTab] = useState<RightTab>("all");
  const [telemetryLog, setTelemetryLog] = useState<string>(
    "ENGINE_READY // ZERO_TELEMETRY · LOCAL_EXECUTION_ONLY",
  );

  const refresh = useCallback(async (label = "refresh") => {
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

  async function setProtection(
    window: WindowInfo,
    enabled: boolean,
    mode: ProtectionMode,
  ) {
    setBusy(`window-${window.id}`);
    try {
      if (enabled) {
        await api.addAppRule(window.app, mode);
        setTelemetryLog(
          `RULE_APPLIED // ${window.app_name} -> ${modeLabel(mode).toUpperCase()}`,
        );
      } else {
        await api.removeAppRules(window.app);
        await api.removeWindowRules(window.id);
        setTelemetryLog(`RULE_REMOVED // ${window.app_name}`);
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
      const report = await api.tick();
      await refresh("tick");
      setTelemetryLog(
        `SHIELD_ARMED // APPLIED:${report.applied.length} UNCHANGED:${report.unchanged} FAILED:${report.failed.length}`,
      );
      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(null);
    }
  }

  async function protectAllWindows() {
    setBusy("protect-all");
    try {
      const mode: ProtectionMode =
        caps?.modes.exclusion.level !== "unsupported" ? "exclusion" : "surface";
      await api.protectAll(mode);
      await refresh("protect-all");
      setTelemetryLog(
        `BULK_SHIELD_ENGAGED // ALL PROTECTABLE APPS -> ${modeLabel(mode).toUpperCase()}`,
      );
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
      const report = await api.releaseAll();
      await refresh("release");
      if (report.failed.length) throw new Error(`Release failed: ${report.failed.join("; ")}`);
      setTelemetryLog(
        `SHIELD_DISARMED // RELEASED ${report.released} WINDOWS (RULES PRESERVED)`,
      );
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
      const res = await api.verify();
      setVerification(res);
      setTelemetryLog(
        `CANARY_AUDIT // OUTCOME:${res.outcome.toUpperCase()} PATH:${res.record.path.toUpperCase()}`,
      );
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
      await api.releaseAll();
      await refresh("reset");
      setTelemetryLog("PROFILE_RESET // ALL RULES PURGED & WINDOWS RESTORED");
      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(null);
    }
  }

  async function setAutoArm(autoArm: AutoArm) {
    setBusy("auto-arm");
    try {
      await api.setAutoArm(autoArm);
      await refresh("auto-arm");
      setTelemetryLog(`AUTO_ARM_POLICY // SET TO ${autoArm.toUpperCase()}`);
      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(null);
    }
  }

  const busyMode = busy !== null;
  const backend = inv?.backend ?? null;
  const windows = inv?.windows ?? [];
  const applied = inv?.applied ?? [];
  const protectedCount = windows.filter((w) =>
    Boolean(findRuleForWindow(profile, w)),
  ).length;
  const isArmed = Boolean(inv?.armed) || applied.length > 0;
  const captureActive =
    Boolean(inv?.capture_active) || windows.some(isCaptureWindow);

  return (
    <TooltipProvider>
      <div className="flex h-full flex-col bg-cyber-grid text-ink">
        <TopBar
          backend={backend}
          platform={caps?.platform ?? null}
          armed={isArmed}
          protectedCount={protectedCount}
          totalWindows={windows.length}
          captureActive={captureActive}
          busy={busyMode}
          onToggleArm={() => {
            if (isArmed) {
              void releaseEverything();
            } else if (protectedCount === 0 && windows.length > 0) {
              void protectAllWindows();
            } else {
              void armNow();
            }
          }}
          onOpenSurface={() =>
            void api.openSurface().catch((err) => setError(String(err)))
          }
        />

        {/* Status / Alert Strip */}
        <div className="space-y-1.5 px-4 pt-2 pb-1.5">
          {backend && (
            <BackendBanner
              tone={backend.live ? "info" : "warning"}
              title={backend.live ? `[${backend.name.toUpperCase()}]` : "[SIMULATION_MODE]"}
            >
              {backend.note}
            </BackendBanner>
          )}
          {error && (
            <BackendBanner tone="error" title="[BACKEND_ERROR]">
              {error}
            </BackendBanner>
          )}
        </div>

        {/* Main Two-Column Systems Deck */}
        <main className="flex min-h-0 flex-1 gap-3 px-4 pb-2">
          {/* Left Column: Window Matrix + Bulk Command Bar */}
          <div className="flex min-w-0 flex-[1.55] flex-col gap-2">
            <WindowList
              inventory={
                inv ?? {
                  windows: [],
                  backend: backend ?? {
                    platform: "linux_x11",
                    name: "",
                    live: false,
                    note: "",
                  },
                  applied: [],
                }
              }
              profile={profile}
              caps={caps}
              busy={busyMode}
              onRefresh={() => void refresh()}
              onToggle={(window) => {
                const rule = findRuleForWindow(profile, window);
                const mode =
                  rule?.mode ??
                  (caps?.modes.exclusion.level !== "unsupported"
                    ? "exclusion"
                    : "surface");
                void setProtection(window, !rule, mode);
              }}
              onMode={(window, mode) => void setProtection(window, true, mode)}
            />

            {/* Bottom Bulk Command Deck */}
            <div className="flex flex-wrap items-center gap-2 rounded-xl border border-line bg-panel/95 px-3 py-2 font-mono shadow-md">
              <Button
                variant="secondary"
                size="sm"
                onClick={() => void protectAllWindows()}
                disabled={busyMode}
                className="h-8 gap-1.5 font-mono text-xs"
              >
                <Sparkles className="size-3.5 text-accent" />
                Protect all
              </Button>
              <Button
                size="sm"
                onClick={() => void armNow()}
                disabled={busyMode}
                className="h-8 gap-1.5 font-mono text-xs"
              >
                <ShieldCheck className="size-3.5" />
                Arm now
              </Button>
              <Button
                variant="secondary"
                size="sm"
                onClick={() => void releaseEverything()}
                disabled={busyMode}
                className="h-8 gap-1.5 font-mono text-xs"
              >
                <ShieldOff className="size-3.5 text-warn" />
                Release all
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => void clearRules()}
                disabled={busyMode}
                className="h-8 gap-1.5 font-mono text-xs text-ink-dim hover:text-bad"
              >
                <Trash2 className="size-3.5" />
                Clear rules
              </Button>

              <div className="ml-auto flex items-center gap-2 text-[11px]">
                <span
                  className={cn(
                    "inline-flex items-center gap-1.5 rounded-md border px-2 py-0.5 font-bold",
                    isArmed
                      ? "border-accent/40 bg-accent/15 text-accent"
                      : "border-line bg-void text-ink-dim",
                  )}
                >
                  <Shield className="size-3" />
                  {applied.length} ACTIVE · {protectedCount} RULED
                </span>
              </div>
            </div>
          </div>

          {/* Right Column: Multi-Module Inspector Deck */}
          <aside className="flex min-w-0 flex-1 flex-col gap-2">
            {/* Module Filter Bar */}
            <div className="flex items-center justify-between gap-1 rounded-xl border border-line bg-panel/95 p-1 font-mono text-[10px]">
              {(
                [
                  { id: "all", label: "ALL_DECK" },
                  { id: "surface", label: "02 // SURFACE" },
                  { id: "verify", label: "03 // CANARY" },
                  { id: "policy", label: "04 // POLICY" },
                ] as { id: RightTab; label: string }[]
              ).map((tab) => (
                <button
                  key={tab.id}
                  type="button"
                  onClick={() => setRightTab(tab.id)}
                  className={cn(
                    "flex-1 whitespace-nowrap rounded-lg px-2 py-1.5 text-center font-bold transition-all cursor-pointer",
                    rightTab === tab.id
                      ? "bg-row text-accent border border-accent/35 shadow-sm"
                      : "text-ink-faint hover:text-ink",
                  )}
                >
                  {tab.label}
                </button>
              ))}
            </div>

            <div className="flex min-h-0 flex-1 flex-col gap-2.5 overflow-y-auto pr-0.5">
              {(rightTab === "all" || rightTab === "surface") && (
                <SurfaceCard
                  preview={preview}
                  busy={busyMode}
                  onRender={() => void refresh("preview")}
                  onOpen={() =>
                    void api.openSurface().catch((err) => setError(String(err)))
                  }
                />
              )}

              {(rightTab === "all" || rightTab === "verify") && (
                <VerificationCard
                  verification={verification}
                  busy={busyMode}
                  onRun={() => void runVerification()}
                />
              )}

              {(rightTab === "all" || rightTab === "policy") && (
                <>
                  <RulesCard
                    profile={profile}
                    busy={busyMode}
                    onToggleRule={async (id, enabled) => {
                      setBusy("rules");
                      try {
                        await api.setRuleEnabled(id, enabled);
                        await api.tick();
                        await refresh("rules");
                        setError(null);
                      } catch (err) {
                        setError(String(err));
                      } finally {
                        setBusy(null);
                      }
                    }}
                    onRemoveRule={async (id) => {
                      setBusy("rules");
                      try {
                        await api.removeRule(id);
                        await api.tick();
                        await refresh("rules");
                        setError(null);
                      } catch (err) {
                        setError(String(err));
                      } finally {
                        setBusy(null);
                      }
                    }}
                    onClear={() => void clearRules()}
                    onAutoArm={(autoArm) => void setAutoArm(autoArm)}
                  />
                  <CapabilitiesCard caps={caps} matrix={matrix} />
                </>
              )}
            </div>
          </aside>
        </main>

        {/* OpenCode-Style Telemetry Footer */}
        <footer className="flex items-center justify-between gap-4 border-t border-line bg-panel/95 px-4 py-1.5 font-mono text-[10px] text-ink-faint">
          <div className="flex items-center gap-3">
            <span className="inline-flex items-center gap-1.5 text-ink-dim">
              <Keyboard className="size-3.5 text-accent" />
              <kbd className="rounded border border-line bg-void px-1.5 py-0.5 text-[9px] text-ink">
                Ctrl
              </kbd>
              +
              <kbd className="rounded border border-line bg-void px-1.5 py-0.5 text-[9px] text-ink">
                Shift
              </kbd>
              +
              <kbd className="rounded border border-line bg-void px-1.5 py-0.5 text-[9px] text-accent">
                V
              </kbd>
              <span className="text-ink-faint">GLOBAL_ARM_TOGGLE</span>
            </span>

            <span className="hidden md:inline-flex items-center gap-1.5 text-cyan">
              <Terminal className="size-3" />
              {telemetryLog}
            </span>
          </div>

          <div className="flex items-center gap-2">
            <span className="inline-flex items-center gap-1 text-good">
              <Activity className="size-3" />
              100% LOCAL · ZERO TELEMETRY
            </span>
          </div>
        </footer>
      </div>
    </TooltipProvider>
  );
}
