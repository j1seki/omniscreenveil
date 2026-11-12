import { useState } from "react";
import { ChevronDown, ShieldCheck, ShieldOff, Trash2 } from "lucide-react";
import type { Profile } from "../types";
import { modeLabel } from "../types";
import { cn } from "../lib/utils";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Switch } from "./ui/switch";

function appLabel(value: string): string {
  const parts = value.replaceAll("\\", "/").split("/");
  return parts[parts.length - 1] || value;
}

export function RulesCard({
  profile,
  busy,
  onToggleRule,
  onRemoveRule,
  onClear,
}: {
  profile: Profile | null;
  busy: boolean;
  onToggleRule: (id: number, enabled: boolean) => void;
  onRemoveRule: (id: number) => void;
  onClear: () => void;
}) {
  const [open, setOpen] = useState(false);
  const rules = profile?.rules ?? [];

  return (
    <section className="rounded-xl border border-line bg-panel">
      <button
        className="flex w-full items-center justify-between gap-3 px-4 py-3 text-left outline-none focus-visible:ring-2 focus-visible:ring-accent/60"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
      >
        <div>
          <h2 className="text-sm font-semibold tracking-tight">Persistent rules</h2>
          <p className="mt-0.5 text-xs text-ink-faint">
            {rules.length === 0
              ? "No rules yet — protect a window to create one"
              : `${rules.length} rule${rules.length === 1 ? "" : "s"} · ${rules.filter((r) => r.enabled).length} active`}
          </p>
        </div>
        <ChevronDown
          className={cn("size-4 text-ink-faint transition-transform", open && "rotate-180")}
        />
      </button>

      {open && (
        <div className="space-y-1 border-t border-line-soft p-2.5">
          {rules.length === 0 && (
            <p className="px-2 py-3 text-xs leading-relaxed text-ink-faint">
              Rules remember your choices across restarts. Protect a window above to create one.
            </p>
          )}
          {rules.map((rule) => (
            <div
              key={rule.id}
              className={cn(
                "flex items-center gap-3 rounded-lg px-2.5 py-2 transition-colors",
                rule.enabled ? "hover:bg-row" : "opacity-50 hover:bg-row",
              )}
            >
              {rule.enabled ? (
                <ShieldCheck className="size-4 shrink-0 text-accent" />
              ) : (
                <ShieldOff className="size-4 shrink-0 text-ink-faint" />
              )}
              <div className="min-w-0 flex-1">
                <p className="truncate text-[13px] font-medium text-ink">
                  {rule.target.kind === "app" ? appLabel(rule.target.value.value) : "Window"}
                </p>
                <p className="truncate font-mono text-[11px] text-ink-faint">
                  {rule.target.kind === "app" ? rule.target.value.value : `0x${rule.target.value.toString(16)}`}
                </p>
              </div>
              <Badge variant={rule.mode === "exclusion" ? "good" : rule.mode === "surface" ? "accent" : "default"}>
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
                className="size-7"
              >
                <Trash2 className="size-3.5" />
              </Button>
            </div>
          ))}
          {rules.length > 0 && (
            <Button variant="ghost" size="sm" onClick={onClear} disabled={busy} className="mt-1 w-full">
              <Trash2 className="size-3.5" />
              Clear all rules
            </Button>
          )}
        </div>
      )}
    </section>
  );
}
