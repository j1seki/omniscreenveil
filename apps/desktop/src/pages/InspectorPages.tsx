import {
  AppWindow,
  CheckCircle2,
  CircleHelp,
  ExternalLink,
  RefreshCw,
  ScanSearch,
  ShieldCheck,
  Trash2,
  XCircle,
} from "lucide-react";
import type {
  AutoArm,
  Capabilities,
  Profile,
  SurfacePreview,
  VerificationRun,
} from "../types";
import { LEVEL_BADGES, MODES, PLATFORM_LABELS, levelDetail, modeLabel } from "../types";
import { cn } from "../lib/utils";
import { CanaryGrid } from "../components/Visuals";
import { Badge } from "../components/ui/badge";
import { Button } from "../components/ui/button";
import { Switch } from "../components/ui/switch";

/* ------------------------------ Surface ---------------------------------- */
export function SurfacePage({
  preview,
  busy,
  onRender,
  onOpen,
}: {
  preview: SurfacePreview | null;
  busy: boolean;
  onRender: () => void;
  onOpen: () => void;
}) {
  return (
    <div className="animate-page-in space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-[14px] font-semibold text-text">Live compositor preview</h2>
          <p className="mt-0.5 text-[12px] text-muted">
            {preview
              ? `${preview.width}×${preview.height} · ${preview.masked} masked region${preview.masked === 1 ? "" : "s"} · ${preview.source}`
              : "Waiting for the first frame…"}
          </p>
        </div>
        <div className="flex gap-2">
          <Button variant="secondary" size="sm" onClick={onRender} disabled={busy} className="h-8 text-[12px]">
            <RefreshCw className={cn("size-3.5", busy && "animate-spin")} />
            Re-render
          </Button>
          <Button size="sm" onClick={onOpen} disabled={busy} className="h-8 text-[12px]">
            <ExternalLink className="size-3.5" />
            Open Surface window
          </Button>
        </div>
      </div>

      <div className="overflow-hidden rounded-xl border border-border bg-panel">
        <div className="flex items-center justify-between border-b border-border px-4 py-2.5">
          <span className="flex items-center gap-2 text-[12px] text-muted">
            <AppWindow className="size-3.5 text-accent" />
            Composite stream
          </span>
          <Badge variant={preview && preview.masked ? "accent" : "default"} className="text-[10px]">
            {preview ? `${preview.masked} veiled` : "idle"}
          </Badge>
        </div>
        <div className="scan-overlay relative aspect-[16/10] bg-app">
          {preview?.png_base64 ? (
            <img
              src={`data:image/png;base64,${preview.png_base64}`}
              alt="Veil Surface preview with protected windows masked"
              className="size-full object-contain"
            />
          ) : (
            <div className="flex h-full items-center justify-center font-mono text-[12px] text-faint">
              [ AWAITING COMPOSITOR FRAME ]
            </div>
          )}
          <div className="pointer-events-none absolute top-3 left-3 rounded border border-border bg-app/85 px-2 py-1 font-mono text-[10px] text-muted backdrop-blur">
            ┌ VEIL_SURFACE // PREVIEW ONLY
          </div>
          <div className="pointer-events-none absolute right-3 bottom-3 rounded border border-border bg-app/85 px-2 py-1 font-mono text-[10px] text-accent backdrop-blur">
            SHARE_SAFE ┘
          </div>
        </div>
      </div>

      <div className="grid gap-3 md:grid-cols-3">
        {[
          {
            title: "Share the Surface",
            text: "In OBS, Discord, Zoom, or your browser share dialog, select the dedicated OmniScreenVeil Surface window — not your whole desktop.",
          },
          {
            title: "Stay in control locally",
            text: "Your protected windows remain visible and interactive on your own screen. Only the composited, masked frame is shown to your audience.",
          },
          {
            title: "Test before you share",
            text: "The preview and audit use synthetic checks. Verify masking in your actual sharing application before exposing sensitive content.",
          },
        ].map((item, i) => (
          <div key={item.title} className="rounded-lg border border-border bg-panel p-3.5">
            <span className="font-mono text-[10px] text-faint">0{i + 1}</span>
            <h3 className="mt-1 text-[12px] font-semibold text-text">{item.title}</h3>
            <p className="mt-1 text-[11px] leading-relaxed text-muted">{item.text}</p>
          </div>
        ))}
      </div>
    </div>
  );
}

/* ------------------------------ Canary ----------------------------------- */
function Outcome({ outcome }: { outcome: VerificationRun["outcome"] }) {
  const map = {
    protected: { icon: CheckCircle2, text: "Self-test passed", cls: "text-success" },
    visible: { icon: XCircle, text: "Token visible", cls: "text-danger" },
    inconclusive: { icon: CircleHelp, text: "Inconclusive", cls: "text-warning" },
  }[outcome];
  const Icon = map.icon;
  return (
    <span className={cn("inline-flex items-center gap-1.5 text-[13px] font-semibold", map.cls)}>
      <Icon className="size-4" />
      {map.text}
    </span>
  );
}

export function AuditPage({
  verification,
  busy,
  onRun,
}: {
  verification: VerificationRun | null;
  busy: boolean;
  onRun: () => void;
}) {
  return (
    <div className="animate-page-in space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-[14px] font-semibold text-text">Synthetic pixel self-test</h2>
          <p className="mt-0.5 text-[12px] text-muted">
            Deterministic 8×8 canary · baseline → veil → re-capture
          </p>
        </div>
        <Button size="sm" onClick={onRun} disabled={busy} className="h-8 text-[12px]">
          <ScanSearch className={cn("size-3.5", busy && "animate-pulse")} />
          {busy ? "Auditing…" : "Run audit"}
        </Button>
      </div>

      <div className="rounded-xl border border-warning/25 bg-warning-soft px-4 py-3 text-[12px] text-muted">
        <strong className="text-warning">Synthetic check.</strong> This composites a canary into an
        in-memory frame and verifies the veil removes it. It does not inspect your real windows,
        OS capture, or the sharing application you use. Test the real capture path before sharing.
      </div>

      <CanaryGrid verification={verification} busy={busy} />

      {verification && (
        <div className="rounded-xl border border-border bg-panel p-4">
          <div className="flex items-center justify-between">
            <Outcome outcome={verification.outcome} />
            <Badge variant="default" className="font-mono text-[10px] uppercase">
              {verification.record.path}
            </Badge>
          </div>
          <div className="mt-3 grid grid-cols-2 gap-3">
            <div className="rounded-lg border border-border bg-app p-3">
              <p className="text-[10px] uppercase tracking-wide text-faint">Baseline</p>
              <p className="mt-1 text-[12px] font-medium text-text">
                {verification.baseline.found ? `Canary visible · ${verification.baseline.matched}/64` : "Not seen"}
              </p>
            </div>
            <div className="rounded-lg border border-border bg-app p-3">
              <p className="text-[10px] uppercase tracking-wide text-faint">After arming</p>
              <p className="mt-1 text-[12px] font-medium text-text">
                {verification.after.found ? `Canary visible · ${verification.after.matched}/64` : `Canary absent · ${verification.after.matched}/64`}
              </p>
            </div>
          </div>
          <p className="mt-3 text-[12px] leading-relaxed text-muted">{verification.record.detail}</p>
          <dl className="mt-3 flex gap-6 border-t border-border pt-3 font-mono text-[11px]">
            <div>
              <dt className="text-faint">CHECKED</dt>
              <dd className="mt-0.5 text-muted">{new Date(verification.record.checked_at_unix * 1000).toLocaleString()}</dd>
            </div>
            <div>
              <dt className="text-faint">BUILD</dt>
              <dd className="mt-0.5 text-muted">{verification.record.os_build}</dd>
            </div>
          </dl>
        </div>
      )}
    </div>
  );
}

/* ------------------------------ Rules ------------------------------------ */
const AUTO_MODES: { id: AutoArm; label: string; hint: string }[] = [
  { id: "off", label: "Manual", hint: "Arm with the button or Ctrl+Shift+V" },
  { id: "capture_app_running", label: "Capture app", hint: "Auto-arm when OBS, Zoom, Discord, etc. run" },
  { id: "always", label: "Always", hint: "Keep protection active continuously" },
];

function appLabel(value: string) {
  const parts = value.replaceAll("\\", "/").split("/");
  return parts[parts.length - 1] || value;
}

export function RulesPage({
  profile,
  busy,
  onAutoArm,
  onToggleRule,
  onRemoveRule,
  onClear,
}: {
  profile: Profile | null;
  busy: boolean;
  onAutoArm: (v: AutoArm) => void;
  onToggleRule: (id: number, enabled: boolean) => void;
  onRemoveRule: (id: number) => void;
  onClear: () => void;
}) {
  const rules = profile?.rules ?? [];
  const active = profile?.auto_arm ?? "off";

  return (
    <div className="animate-page-in space-y-4">
      <div className="rounded-xl border border-border bg-panel p-4">
        <div className="flex items-center justify-between">
          <div>
            <h3 className="text-[13px] font-semibold text-text">Auto-arm policy</h3>
            <p className="mt-0.5 text-[12px] text-muted">
              {AUTO_MODES.find((m) => m.id === active)?.hint}
            </p>
          </div>
        </div>
        <div className="mt-3 grid grid-cols-3 gap-2">
          {AUTO_MODES.map((m) => (
            <button
              key={m.id}
              type="button"
              disabled={busy}
              onClick={() => onAutoArm(m.id)}
              className={cn(
                "rounded-lg border px-3 py-2.5 text-left transition-all disabled:opacity-50",
                active === m.id
                  ? "border-accent/50 bg-accent-soft"
                  : "border-border bg-app hover:border-border-strong",
              )}
            >
              <span className={cn("block text-[12px] font-medium", active === m.id ? "text-accent" : "text-text")}>
                {m.label}
              </span>
              <span className="mt-0.5 block text-[10px] text-faint">{m.hint}</span>
            </button>
          ))}
        </div>
        <p className="mt-2 text-[11px] text-faint">
          A running capture app is a hint, never proof of an active share. Release all switches auto-arm off.
        </p>
      </div>

      <div className="overflow-hidden rounded-xl border border-border bg-panel">
        <div className="flex items-center justify-between border-b border-border px-4 py-3">
          <div>
            <h3 className="text-[13px] font-semibold text-text">Persistent rules</h3>
            <p className="mt-0.5 text-[11px] text-faint">
              {rules.length === 0 ? "No rules stored" : `${rules.length} rule${rules.length === 1 ? "" : "s"} · ${rules.filter((r) => r.enabled).length} active`}
            </p>
          </div>
          {rules.length > 0 && (
            <Button variant="ghost" size="sm" onClick={onClear} disabled={busy} className="h-7 text-[11px] text-muted hover:text-danger">
              <Trash2 className="size-3" />
              Purge all
            </Button>
          )}
        </div>
        {rules.length === 0 ? (
          <div className="flex flex-col items-center gap-2 px-4 py-12 text-center">
            <ShieldCheck className="size-5 text-faint" />
            <p className="text-[12px] font-medium text-text">No persistent rules yet</p>
            <p className="max-w-[300px] text-[11px] text-faint">
              Toggle protection for a window in the Windows page to create a rule that survives restarts.
            </p>
          </div>
        ) : (
          <div className="divide-y divide-border/70">
            {rules.map((rule) => (
              <div key={rule.id} className="flex items-center gap-3 px-4 py-2.5">
                <ShieldCheck className={cn("size-4 shrink-0", rule.enabled ? "text-success" : "text-faint")} />
                <div className="min-w-0 flex-1">
                  <p className="truncate text-[12px] font-medium text-text">
                    {rule.target.kind === "app"
                      ? appLabel(rule.target.value.value)
                      : `Window 0x${rule.target.value.toString(16)}`}
                  </p>
                  <p className="truncate font-mono text-[10px] text-faint">
                    {rule.target.kind === "app" ? rule.target.value.value : "one-shot window handle"}
                  </p>
                </div>
                <Badge
                  variant={rule.mode === "exclusion" ? "live" : rule.mode === "surface" ? "accent" : "sim"}
                  className="text-[10px]"
                >
                  {modeLabel(rule.mode)}
                </Badge>
                <Switch
                  checked={rule.enabled}
                  disabled={busy}
                  onCheckedChange={(v) => onToggleRule(rule.id, v)}
                  aria-label={`Enable rule ${rule.id}`}
                />
                <button
                  type="button"
                  onClick={() => onRemoveRule(rule.id)}
                  disabled={busy}
                  className="text-faint transition-colors hover:text-danger"
                >
                  <Trash2 className="size-3.5" />
                </button>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

/* --------------------------- Capabilities -------------------------------- */
export function CapabilitiesPage({
  caps,
  matrix,
}: {
  caps: Capabilities | null;
  matrix: Capabilities[];
}) {
  return (
    <div className="animate-page-in space-y-4">
      <div className="grid gap-3 md:grid-cols-3">
        {MODES.map((mode) => {
          const level = caps?.modes[mode.id];
          return (
            <div key={mode.id} className="rounded-xl border border-border bg-panel p-4">
              <div className="flex items-center justify-between">
                <span className="text-[13px] font-semibold text-text">{mode.label}</span>
                <Badge
                  variant={
                    level?.level === "native" ? "live" : level?.level === "partial" ? "sim" : "default"
                  }
                  className="text-[10px] capitalize"
                >
                  {level?.level ?? "…"}
                </Badge>
              </div>
              <p className="mt-2 text-[11px] leading-relaxed text-muted">
                {level ? levelDetail(level) : "Detecting…"}
              </p>
            </div>
          );
        })}
      </div>

      <div className="overflow-hidden rounded-xl border border-border bg-panel">
        <div className="border-b border-border px-4 py-3">
          <h3 className="text-[13px] font-semibold text-text">Cross-platform support matrix</h3>
          <p className="mt-0.5 text-[11px] text-faint">
            ✓ native · ◐ partial · — unsupported. No capability is claimed without a test.
          </p>
        </div>
        <table className="w-full text-[12px]">
          <thead>
            <tr className="border-b border-border text-left text-[10px] uppercase tracking-wide text-faint">
              <th className="px-4 py-2 font-medium">Mode</th>
              {matrix.map((entry) => (
                <th key={entry.platform} className="px-3 py-2 text-center font-medium">
                  {PLATFORM_LABELS[entry.platform]}
                </th>
              ))}
            </tr>
          </thead>
          <tbody className="divide-y divide-border/70">
            {MODES.map((mode) => (
              <tr key={mode.id}>
                <td className="px-4 py-2.5 text-muted">{mode.label}</td>
                {matrix.map((entry) => {
                  const level = entry.modes[mode.id];
                  return (
                    <td
                      key={entry.platform}
                      className={cn(
                        "px-3 py-2.5 text-center font-semibold",
                        level.level === "native" && "text-success",
                        level.level === "partial" && "text-warning",
                        level.level === "unsupported" && "text-faint",
                      )}
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
      </div>
    </div>
  );
}
