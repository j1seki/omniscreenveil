import { useState } from "react";
import { ChevronDown, ShieldCheck, ShieldOff, Trash2, Zap } from "lucide-react";
import type { AutoArm, Profile } from "../types";
import { modeLabel } from "../types";
import { cn } from "../lib/utils";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Switch } from "./ui/switch";

function appLabel(value: string): string {
  const parts = value.replaceAll("\\", "/").split("/");
  return parts[parts.length - 1] || value;
}

const AUTO_ARM_MODES: { id: AutoArm; label: string; hint: string }[] = [
  { id: "off", label: "MANUAL", hint: "Arm via button or Ctrl+Shift+V" },
  {
    id: "capture_app_running",
    label: "ON_CAPTURE_APP",
    hint: "Auto-arm when OBS, Discord, Zoom, ShareX runs",
  },
  { id: "always", label: "ALWAYS_ON", hint: "Continuous zero-trust protection" },
];

export function RulesCard({
  profile,
  busy,
  onToggleRule,
  onRemoveRule,
  onClear,
  onAutoArm,
}: {
  profile: Profile | null;
  busy: boolean;
  onToggleRule: (id: number, enabled: boolean) => void;
  onRemoveRule: (id: number) => void;
  onClear: () => void;
  onAutoArm: (autoArm: AutoArm) => void;
}) {
  const [open, setOpen] = useState(true);
  const rules = profile?.rules ?? [];
  const activeAutoArm = profile?.auto_arm ?? "off";

  return (
    <section className="rounded-xl border border-line bg-panel/95 shadow-lg">
      <button
        type="button"
        className="flex w-full items-center justify-between gap-2 px-3.5 py-2.5 text-left outline-none focus-visible:ring-2 focus-visible:ring-accent/60 cursor-pointer"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
      >
        <div className="flex min-w-0 items-center gap-2.5">
          <span className="shrink-0 whitespace-nowrap rounded border border-warn/40 bg-warn/10 px-1.5 py-0.5 font-mono text-[10px] font-bold text-warn">
            04 // RULES
          </span>
          <div>
            <h2 className="font-mono text-xs font-bold tracking-wide text-ink uppercase">
              Policy & Auto-Arm Engine
            </h2>
            <p className="font-mono text-[10px] text-ink-faint">
              {rules.length === 0
                ? "0 PERSISTENT RULES CONFIGURED"
                : `${rules.length} RULE${rules.length === 1 ? "" : "S"} · ${rules.filter((r) => r.enabled).length} ACTIVE`}
            </p>
          </div>
        </div>
        <ChevronDown
          className={cn(
            "size-4 text-ink-faint transition-transform",
            open && "rotate-180",
          )}
        />
      </button>

      {/* Tactile Auto-Arm Policy Selector */}
      <div className="border-t border-line-soft px-3.5 py-2.5">
        <div className="mb-1.5 flex items-center justify-between font-mono text-[10px]">
          <span className="inline-flex items-center gap-1 font-semibold text-ink-dim">
            <Zap className="size-3 text-warn" />
            AUTO_ARM_POLICY
          </span>
          <span className="text-ink-faint">
            {AUTO_ARM_MODES.find((m) => m.id === activeAutoArm)?.hint}
          </span>
        </div>

        <div className="grid grid-cols-3 gap-1 rounded-lg border border-line bg-void p-1 font-mono text-[10px]">
          {AUTO_ARM_MODES.map((item) => {
            const active = activeAutoArm === item.id;
            return (
              <button
                key={item.id}
                type="button"
                disabled={busy}
                onClick={() => onAutoArm(item.id)}
                className={cn(
                  "rounded-md px-2 py-1.5 text-center transition-all cursor-pointer disabled:opacity-50",
                  active
                    ? "bg-accent text-void font-bold shadow-[0_0_12px_rgba(0,245,212,0.3)]"
                    : "text-ink-dim hover:bg-row hover:text-ink",
                )}
              >
                {item.label}
              </button>
            );
          })}
        </div>
        <p className="mt-1.5 font-mono text-[9px] text-ink-faint">
          A running capture app is only a hint. Release all switches auto-arm off.
        </p>
      </div>

      {open && (
        <div className="space-y-1 border-t border-line-soft p-2.5">
          {rules.length === 0 && (
            <p className="px-2 py-2 text-center font-mono text-[10px] text-ink-faint">
              [ NO_RULES_STORED — TOGGLE ANY WINDOW IN THE MATRIX TO PERSIST ]
            </p>
          )}
          {rules.map((rule) => (
            <div
              key={rule.id}
              className={cn(
                "flex items-center gap-2.5 rounded-lg border px-2.5 py-1.5 font-mono transition-colors",
                rule.enabled
                  ? "border-line-soft bg-void/60 hover:bg-row"
                  : "border-transparent bg-void/30 opacity-50 hover:bg-row",
              )}
            >
              {rule.enabled ? (
                <ShieldCheck className="size-3.5 shrink-0 text-accent" />
              ) : (
                <ShieldOff className="size-3.5 shrink-0 text-ink-faint" />
              )}
              <div className="min-w-0 flex-1">
                <p className="truncate text-[11px] font-bold text-ink">
                  {rule.target.kind === "app"
                    ? appLabel(rule.target.value.value)
                    : `Window 0x${rule.target.value.toString(16)}`}
                </p>
                <p className="truncate text-[9px] text-ink-faint">
                  {rule.target.kind === "app"
                    ? rule.target.value.value
                    : `One-shot handle 0x${rule.target.value.toString(16)}`}
                </p>
              </div>
              <Badge
                variant={
                  rule.mode === "exclusion"
                    ? "good"
                    : rule.mode === "surface"
                      ? "accent"
                      : "sim"
                }
                className="text-[9px] uppercase"
              >
                {modeLabel(rule.mode)}
              </Badge>
              <Switch
                checked={rule.enabled}
                disabled={busy}
                aria-label={`Enable rule ${rule.id}`}
                onCheckedChange={(checked) => onToggleRule(rule.id, checked)}
              />
              <Button
                variant="ghost"
                size="icon"
                disabled={busy}
                aria-label={`Delete rule ${rule.id}`}
                onClick={() => onRemoveRule(rule.id)}
                className="size-6 text-ink-faint hover:text-bad"
              >
                <Trash2 className="size-3" />
              </Button>
            </div>
          ))}
          {rules.length > 0 && (
            <Button
              variant="ghost"
              size="sm"
              onClick={onClear}
              disabled={busy}
              className="mt-1 h-7 w-full font-mono text-[10px] text-ink-dim hover:text-bad"
            >
              <Trash2 className="size-3" />
              PURGE ALL RULES
            </Button>
          )}
        </div>
      )}
    </section>
  );
}
