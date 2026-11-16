import { useEffect, useMemo, useRef, useState } from "react";
import { CornerDownLeft, Search } from "lucide-react";
import { cn } from "../lib/utils";

export interface Command {
  id: string;
  label: string;
  group: string;
  icon: React.ComponentType<{ className?: string }>;
  keywords?: string;
  shortcut?: string;
  danger?: boolean;
  run: () => void;
}

export function CommandPalette({
  open,
  onClose,
  commands,
}: {
  open: boolean;
  onClose: () => void;
  commands: Command[];
}) {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return commands;
    return commands.filter((cmd) =>
      `${cmd.label} ${cmd.group} ${cmd.keywords ?? ""}`.toLowerCase().includes(q),
    );
  }, [commands, query]);

  const grouped = useMemo(() => {
    const map = new Map<string, Command[]>();
    for (const cmd of filtered) {
      const list = map.get(cmd.group) ?? [];
      list.push(cmd);
      map.set(cmd.group, list);
    }
    return [...map.entries()];
  }, [filtered]);

  useEffect(() => {
    if (open) {
      setQuery("");
      setSelected(0);
      const t = window.setTimeout(() => inputRef.current?.focus(), 30);
      return () => window.clearTimeout(t);
    }
  }, [open]);

  useEffect(() => {
    setSelected(0);
  }, [query]);

  useEffect(() => {
    const node = listRef.current?.querySelector<HTMLElement>('[data-selected="true"]');
    node?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  if (!open) return null;

  function run(cmd: Command | undefined) {
    if (!cmd) return;
    onClose();
    cmd.run();
  }

  function onKeyDown(e: React.KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelected((s) => Math.min(s + 1, filtered.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelected((s) => Math.max(s - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      run(filtered[selected]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 px-4 pt-[12vh] backdrop-blur-sm animate-fade-in"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Command palette"
        className="w-full max-w-[540px] overflow-hidden rounded-xl border border-border-strong bg-panel shadow-2xl shadow-black/40 animate-scale-in"
        onKeyDown={onKeyDown}
      >
        <div className="flex h-12 items-center gap-3 border-b border-border px-4">
          <Search className="size-4 shrink-0 text-faint" />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Type a command or search…"
            className="h-full flex-1 bg-transparent text-[14px] text-text outline-none placeholder:text-faint"
          />
          <kbd className="rounded border border-border bg-elevated px-1.5 py-0.5 font-mono text-[10px] text-faint">
            ESC
          </kbd>
        </div>

        <div ref={listRef} className="max-h-[min(420px,55vh)] overflow-y-auto p-2">
          {grouped.length === 0 && (
            <div className="flex flex-col items-center gap-2 py-12 text-center">
              <Search className="size-5 text-faint" />
              <p className="text-[13px] text-muted">No commands found</p>
              <p className="text-[11px] text-faint">Try a different search term</p>
            </div>
          )}
          {grouped.map(([group, cmds]) => (
            <div key={group} className="mb-1.5 last:mb-0">
              <p className="px-2.5 py-1.5 text-[10px] font-semibold uppercase tracking-[0.08em] text-faint">
                {group}
              </p>
              {cmds.map((cmd) => {
                const index = filtered.indexOf(cmd);
                const isSelected = index === selected;
                const Icon = cmd.icon;
                return (
                  <button
                    key={cmd.id}
                    type="button"
                    data-selected={isSelected}
                    onMouseEnter={() => setSelected(index)}
                    onClick={() => run(cmd)}
                    className={cn(
                      "flex h-10 w-full items-center gap-3 rounded-md px-2.5 text-left text-[13px] transition-colors",
                      isSelected ? "bg-accent-soft text-text" : "text-muted hover:bg-hover",
                    )}
                  >
                    <span
                      className={cn(
                        "flex size-7 items-center justify-center rounded-md border",
                        isSelected
                          ? "border-accent/30 bg-accent/10 text-accent"
                          : "border-border bg-elevated text-faint",
                        cmd.danger && "text-danger",
                      )}
                    >
                      <Icon className="size-3.5" />
                    </span>
                    <span className="flex-1">{cmd.label}</span>
                    {cmd.shortcut && (
                      <kbd className="font-mono text-[10px] text-faint">{cmd.shortcut}</kbd>
                    )}
                    {isSelected && <CornerDownLeft className="size-3.5 text-accent" />}
                  </button>
                );
              })}
            </div>
          ))}
        </div>

        <div className="flex items-center justify-between border-t border-border px-4 py-2 text-[10px] text-faint">
          <span className="flex items-center gap-3">
            <span>↑↓ navigate</span>
            <span>↵ select</span>
            <span>esc close</span>
          </span>
          <span>{filtered.length} commands</span>
        </div>
      </div>
    </div>
  );
}
