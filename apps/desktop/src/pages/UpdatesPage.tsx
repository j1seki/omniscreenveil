import {
  AlertTriangle,
  CheckCircle2,
  Download,
  RefreshCw,
  RotateCcw,
  ShieldCheck,
} from "lucide-react";
import type { UpdateChannel, UpdateInfo, UpdateProgress } from "../types";
import { cn } from "../lib/utils";
import { BackendBanner } from "../components/BackendBanner";
import { Badge } from "../components/ui/badge";
import { Button } from "../components/ui/button";
import { Switch } from "../components/ui/switch";

const CHANNELS: {
  id: UpdateChannel;
  label: string;
  hint: string;
}[] = [
  {
    id: "stable",
    label: "Stable",
    hint: "Final releases only. Recommended for everyday use.",
  },
  {
    id: "beta",
    label: "Beta",
    hint: "Release candidates and betas before they are promoted to stable.",
  },
];

function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 KB";
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function UpdatesPage({
  supported,
  currentVersion,
  channel,
  setChannel,
  autoCheck,
  setAutoCheck,
  lastChecked,
  update,
  manifestFound,
  checking,
  installing,
  progress,
  installedVersion,
  error,
  check,
  install,
  restart,
}: {
  supported: boolean;
  currentVersion: string;
  channel: UpdateChannel;
  setChannel: (channel: UpdateChannel) => void;
  autoCheck: boolean;
  setAutoCheck: (enabled: boolean) => void;
  lastChecked: number | null;
  update: UpdateInfo | null;
  manifestFound: boolean | null;
  checking: boolean;
  installing: boolean;
  progress: UpdateProgress | null;
  installedVersion: string | null;
  error: string | null;
  check: () => void;
  install: () => void;
  restart: () => void;
}) {
  const percent =
    progress?.total && progress.total > 0
      ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100))
      : null;

  return (
    <div className="animate-page-in space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-[14px] font-semibold text-text">Release channel</h2>
          <p className="mt-0.5 text-[12px] text-muted">
            Automatic checks contact GitHub Releases. Installing always asks first.
          </p>
        </div>
        <Button
          variant="secondary"
          size="sm"
          onClick={check}
          disabled={!supported || checking || installing || Boolean(installedVersion)}
          className="h-8 text-[12px]"
        >
          <RefreshCw className={cn("size-3.5", checking && "animate-spin")} />
          {checking ? "Checking…" : "Check now"}
        </Button>
      </div>

      {!supported && (
        <BackendBanner tone="warning" title="Preview mode">
          Update checks are disabled in the browser preview. Run the desktop app to use the
          updater.
        </BackendBanner>
      )}

      <div className="grid gap-3 md:grid-cols-2">
        {CHANNELS.map((entry) => {
          const selected = channel === entry.id;
          return (
            <button
              key={entry.id}
              type="button"
              onClick={() => setChannel(entry.id)}
              disabled={installing}
              className={cn(
                "rounded-xl border p-4 text-left transition-all disabled:opacity-50",
                selected
                  ? "border-accent/50 bg-accent-soft"
                  : "border-border bg-panel hover:border-border-strong",
              )}
            >
              <span className="flex items-center justify-between gap-2">
                <span
                  className={cn(
                    "text-[13px] font-semibold",
                    selected ? "text-accent" : "text-text",
                  )}
                >
                  {entry.label}
                </span>
                <Badge variant={selected ? "accent" : "default"} className="uppercase">
                  {entry.id}
                </Badge>
              </span>
              <span className="mt-1.5 block text-[11px] leading-relaxed text-muted">
                {entry.hint}
              </span>
            </button>
          );
        })}
      </div>

      <div className="rounded-xl border border-border bg-panel p-4">
        <div className="flex items-center justify-between gap-4">
          <div>
            <h3 className="text-[13px] font-semibold text-text">Check for updates automatically</h3>
            <p className="mt-0.5 text-[11px] text-muted">
              Checks shortly after launch and every six hours. Never downloads without asking.
            </p>
          </div>
          <Switch
            checked={autoCheck}
            onCheckedChange={setAutoCheck}
            disabled={!supported}
            aria-label="Check for updates automatically"
          />
        </div>
        <p className="mt-2 border-t border-border pt-2 text-[11px] text-faint">
          {lastChecked
            ? `Last checked ${new Date(lastChecked).toLocaleString()}`
            : "Not checked yet"}
        </p>
      </div>

      <div className="overflow-hidden rounded-xl border border-border bg-panel">
        <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border px-4 py-3">
          <div>
            <h3 className="text-[13px] font-semibold text-text">This installation</h3>
            <p className="mt-0.5 font-mono text-[11px] text-faint">
              current v{currentVersion || "…"} · channel {channel}
            </p>
          </div>
          <Badge variant="default" className="uppercase">
            {channel}
          </Badge>
        </div>

        <div className="space-y-3 p-4">
          {error && <BackendBanner tone="error" title="Update check failed">{error}</BackendBanner>}

          {installedVersion ? (
            <div className="rounded-lg border border-success/25 bg-success-soft p-4">
              <div className="flex items-center gap-2 text-success">
                <CheckCircle2 className="size-4" />
                <span className="text-[12px] font-semibold">
                  OmniScreenVeil {installedVersion} installed
                </span>
              </div>
              <p className="mt-1.5 text-[11px] leading-relaxed text-muted">
                Restart to run the new version. Any active protection is released before the app
                exits, so protected windows are restored first.
              </p>
              <Button size="sm" onClick={restart} className="mt-3 h-8 text-[12px]">
                <RotateCcw className="size-3.5" />
                Restart OmniScreenVeil
              </Button>
            </div>
          ) : update ? (
            <div className="rounded-lg border border-accent/25 bg-accent-soft p-4">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <div className="flex items-center gap-2">
                  <Download className="size-4 text-accent" />
                  <span className="text-[13px] font-semibold text-text">
                    v{update.version} is available
                  </span>
                  <Badge variant="accent" className="uppercase">
                    {update.channel}
                  </Badge>
                </div>
                {update.date_unix && (
                  <span className="text-[11px] text-faint">
                    {new Date(update.date_unix * 1000).toLocaleDateString()}
                  </span>
                )}
              </div>
              <p className="mt-1 text-[11px] text-muted">
                You are on v{update.current_version}. The download is signed and verified before
                installation.
              </p>
              {update.notes && (
                <div className="mt-2 max-h-44 overflow-y-auto whitespace-pre-wrap rounded-md border border-border bg-app p-3 font-mono text-[11px] leading-relaxed text-muted">
                  {update.notes}
                </div>
              )}

              {installing ? (
                <div className="mt-3 space-y-1.5">
                  <div className="flex items-center justify-between text-[11px] text-muted">
                    <span>
                      {progress?.phase === "installing"
                        ? "Installing update…"
                        : progress?.phase === "installed"
                          ? "Finishing…"
                          : `Downloading ${formatBytes(progress?.downloaded ?? 0)}${
                              progress?.total ? ` of ${formatBytes(progress.total)}` : ""
                            }`}
                    </span>
                    {percent !== null && <span>{percent}%</span>}
                  </div>
                  <div className="h-1.5 overflow-hidden rounded-full bg-border">
                    <div
                      className={cn(
                        "h-full rounded-full bg-accent transition-[width] duration-300",
                        percent === null && "w-1/3 animate-pulse",
                      )}
                      style={percent !== null ? { width: `${percent}%` } : undefined}
                    />
                  </div>
                </div>
              ) : (
                <Button size="sm" onClick={install} className="mt-3 h-8 text-[12px]">
                  <Download className="size-3.5" />
                  Download and install
                </Button>
              )}
            </div>
          ) : (
            <div className="flex items-start gap-2.5 rounded-lg border border-border bg-app px-3 py-2.5 text-[11px] leading-snug text-muted">
              {supported && manifestFound === true ? (
                <ShieldCheck className="mt-0.5 size-3.5 shrink-0 text-success" />
              ) : supported && (checking || manifestFound === null) ? (
                <RefreshCw
                  className={cn("mt-0.5 size-3.5 shrink-0", checking && "animate-spin")}
                />
              ) : (
                <AlertTriangle className="mt-0.5 size-3.5 shrink-0 text-warning" />
              )}
              <span>
                <strong className="font-semibold text-text">
                  {!supported
                    ? "Preview"
                    : checking
                      ? "Checking…"
                      : manifestFound === false
                        ? "Channel not published"
                        : manifestFound === true
                          ? "Up to date"
                          : "Not checked yet"}
                </strong>{" "}
                {!supported
                  ? "Update checks are available in the packaged desktop app only."
                  : checking
                    ? `Contacting GitHub for the latest ${channel} release.`
                    : manifestFound === false
                      ? `No ${channel} update manifest exists yet. This is normal before the first ${channel} release is published with the new workflow.`
                      : manifestFound === true
                        ? `No ${channel} update is available for v${currentVersion || "this build"}.`
                        : autoCheck
                          ? "Automatic checks run shortly after launch and every six hours."
                          : "Use Check now to look for a newer release."}
              </span>
            </div>
          )}
        </div>
      </div>

      <div className="rounded-xl border border-border bg-panel p-4 text-[11px] leading-relaxed text-muted">
        <div className="flex items-center gap-2 text-[12px] font-semibold text-text">
          <ShieldCheck className="size-3.5 text-accent" />
          How channels map to GitHub
        </div>
        <p className="mt-1.5">
          Stable follows the latest non-prerelease GitHub release. Beta follows a rolling
          <span className="font-mono text-text"> updater-beta</span> release that is refreshed from
          every SemVer pre-release tag (for example <span className="font-mono">v0.2.0-rc.1</span>).
          Switching from beta to stable will not downgrade an installed beta; the next newer stable
          release replaces it.
        </p>
        <p className="mt-1.5 text-faint">
          This app has no telemetry. The only network request it makes is the update check against
          github.com when you enable automatic checks or press Check now.
        </p>
      </div>
    </div>
  );
}
