import { useCallback, useEffect, useState } from "react";
import { Keyboard, ShieldOff } from "lucide-react";
import * as api from "./api";
import type {
  Capabilities,
  Inventory,
  Profile,
  ProtectionMode,
  SurfacePreview,
  VerificationRun,
  WindowInfo,
} from "./types";
import { BackendBanner } from "./components/BackendBanner";
import { CapabilitiesCard } from "./components/CapabilitiesCard";
import { RulesCard } from "./components/RulesCard";
import { SurfaceCard } from "./components/SurfaceCard";
import { TopBar } from "./components/TopBar";
import { VerificationCard } from "./components/VerificationCard";
import { WindowList } from "./components/WindowList";
import { Button } from "./components/ui/button";
import { TooltipProvider } from "./components/ui/tooltip";

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

  async function setProtection(window: WindowInfo, enabled: boolean, mode: ProtectionMode) {
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
  const backend = inv?.backend ?? null;
  const protectedCount = inv?.windows.filter((w) => {
    const rule = profile?.rules.find(
      (r) =>
        r.enabled &&
        r.target.kind === "app" &&
        r.target.value.value.replaceAll("\\", "/").toLowerCase() ===
          w.app.value.replaceAll("\\", "/").toLowerCase(),
    );
    return rule;
  }).length ?? 0;

  return (
    <TooltipProvider>
      <div className="flex h-full flex-col bg-void">
        <TopBar backend={backend} platform={caps?.platform ?? null} />

        <div className="space-y-2.5 px-5 pb-2.5">
          {backend && (
            <BackendBanner
              tone={backend.live ? "info" : "warning"}
              title={backend.live ? "Live backend." : "Preview / simulation."}
            >
              {backend.note}
            </BackendBanner>
          )}
          {error && (
            <BackendBanner tone="error" title="Command failed.">
              {error}
            </BackendBanner>
          )}
        </div>

        <main className="flex min-h-0 flex-1 gap-2.5 px-5 pb-2.5">
          <div className="flex min-w-0 flex-[1.6] flex-col gap-2.5">
            <WindowList
              inventory={inv ?? { windows: [], backend: backend ?? { platform: "linux_x11", name: "", live: false, note: "" }, applied: [] }}
              profile={profile}
              caps={caps}
              busy={busyMode}
              onRefresh={() => void refresh()}
              onToggle={(window) => {
                const rule = profile?.rules.find(
                  (r) =>
                    r.enabled &&
                    r.target.kind === "app" &&
                    r.target.value.value.replaceAll("\\", "/").toLowerCase() ===
                      window.app.value.replaceAll("\\", "/").toLowerCase(),
                );
                const mode =
                  rule?.mode ??
                  (caps?.modes.exclusion.level !== "unsupported"
                    ? "exclusion"
                    : "surface");
                void setProtection(window, !rule, mode);
              }}
              onMode={(window, mode) => void setProtection(window, true, mode)}
            />
            <div className="flex items-center gap-2">
              <Button onClick={() => void armNow()} disabled={busyMode}>
                Arm now
              </Button>
              <Button variant="secondary" onClick={() => void releaseEverything()} disabled={busyMode}>
                <ShieldOff className="size-4" />
                Release all
              </Button>
              <Button variant="ghost" onClick={() => void clearRules()} disabled={busyMode}>
                Clear rules
              </Button>
              <span className="ml-auto text-xs text-ink-faint">
                {protectedCount} protected
              </span>
            </div>
          </div>

          <aside className="flex min-w-0 flex-1 flex-col gap-2.5 overflow-y-auto">
            <VerificationCard
              verification={verification}
              busy={busyMode}
              onRun={() => void runVerification()}
            />
            <SurfaceCard
              preview={preview}
              busy={busyMode}
              onRender={() => void refresh("preview")}
              onOpen={() => void api.openSurface().catch((err) => setError(String(err)))}
            />
            <RulesCard
              profile={profile}
              busy={busyMode}
              onToggleRule={async (id, enabled) => {
                setBusy("rules");
                try {
                  await api.setRuleEnabled(id, enabled);
                  await api.tick();
                  await refresh("rules");
                } finally {
                  setBusy(null);
                }
              }}
              onRemoveRule={async (id) => {
                setBusy("rules");
                try {
                  const rule = profile?.rules.find((r) => r.id === id);
                  if (rule?.target.kind === "app") {
                    await api.removeAppRules(rule.target.value);
                  }
                  await api.tick();
                  await refresh("rules");
                } finally {
                  setBusy(null);
                }
              }}
              onClear={() => void clearRules()}
            />
            <CapabilitiesCard caps={caps} matrix={matrix} />
          </aside>
        </main>

        <footer className="flex items-center justify-between gap-4 border-t border-line-soft px-5 py-2 text-[11px] text-ink-faint">
          <span className="inline-flex items-center gap-1.5">
            <Keyboard className="size-3.5" />
            <kbd className="rounded border border-line bg-row px-1.5 py-0.5 font-sans">Ctrl</kbd>
            +
            <kbd className="rounded border border-line bg-row px-1.5 py-0.5 font-sans">Shift</kbd>
            +
            <kbd className="rounded border border-line bg-row px-1.5 py-0.5 font-sans">V</kbd>
            arm / release
          </span>
          <span>No telemetry. No network. Everything runs locally.</span>
        </footer>
      </div>
    </TooltipProvider>
  );
}
