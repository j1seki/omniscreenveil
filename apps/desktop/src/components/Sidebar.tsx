import {
  Activity,
  CircleHelp,
  Command,
  Download,
  LayoutDashboard,
  ListFilter,
  Moon,
  PanelsTopLeft,
  ScanSearch,
  Settings2,
  ShieldCheck,
  Sun,
  Waypoints,
} from "lucide-react";
import type { BackendInfo, Theme } from "../types";
import { cn } from "../lib/utils";

export type Page =
  | "overview"
  | "windows"
  | "surface"
  | "audit"
  | "rules"
  | "capabilities"
  | "settings"
  | "updates";

const NAV: {
  id: Page;
  label: string;
  icon: React.ComponentType<{ className?: string }>;
  group: string;
  keys: string;
}[] = [
  { id: "overview", label: "Overview", icon: LayoutDashboard, group: "Workspace", keys: "1" },
  { id: "windows", label: "Windows", icon: ListFilter, group: "Workspace", keys: "2" },
  { id: "surface", label: "Surface", icon: PanelsTopLeft, group: "Workspace", keys: "3" },
  { id: "audit", label: "Capture test", icon: ScanSearch, group: "Verify", keys: "4" },
  { id: "rules", label: "Rules & automation", icon: Waypoints, group: "Verify", keys: "5" },
  { id: "capabilities", label: "Capabilities", icon: Settings2, group: "Verify", keys: "6" },
  { id: "updates", label: "Updates", icon: Download, group: "System", keys: "7" },
  { id: "settings", label: "Settings", icon: Settings2, group: "System", keys: "8" },
];

function NavItem({
  active,
  icon: Icon,
  label,
  keys,
  dot,
  onClick,
}: {
  active: boolean;
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  keys: string;
  dot?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "group relative flex h-8 w-full items-center gap-2.5 rounded-md px-2.5 text-[13px] transition-colors",
        active
          ? "bg-active font-medium text-text"
          : "text-muted hover:bg-hover hover:text-text",
      )}
    >
      {active && (
        <span className="absolute -left-2 top-1.5 h-5 w-0.5 rounded-full bg-accent" />
      )}
      <Icon className={cn("size-4 shrink-0", active ? "text-accent" : "")} />
      <span className="flex-1 text-left">{label}</span>
      {dot && (
        <span
          className="size-1.5 shrink-0 rounded-full bg-accent"
          title="An update is available"
        />
      )}
      <kbd className="hidden font-mono text-[10px] text-faint group-hover:inline">
        ⌘{keys}
      </kbd>
    </button>
  );
}

export function Sidebar({
  page,
  onNavigate,
  theme,
  onToggleTheme,
  onOpenPalette,
  backend,
  armed,
  protectedCount,
  version,
  updateAvailable,
  onToggleArm,
}: {
  page: Page;
  onNavigate: (page: Page) => void;
  theme: Theme;
  onToggleTheme: () => void;
  onOpenPalette: () => void;
  backend: BackendInfo | null;
  armed: boolean;
  protectedCount: number;
  version: string;
  updateAvailable: boolean;
  onToggleArm: () => void;
}) {
  const live = backend?.live ?? false;
  let lastGroup = "";

  return (
    <aside className="flex w-[228px] shrink-0 flex-col border-r border-border bg-sidebar">
      {/* App identity */}
      <div className="flex h-14 items-center gap-2.5 px-4">
        <div className="flex size-7 items-center justify-center rounded-lg border border-border-strong bg-elevated shadow-sm">
          <img src="/veil-mark.svg" alt="" className="size-6" />
        </div>
        <div className="min-w-0 flex-1">
          <p className="truncate text-[13px] font-semibold tracking-tight text-text">
            OmniScreenVeil
          </p>
          <p className="truncate text-[10px] text-faint">Capture privacy · v{version || "…"}</p>
        </div>
        <button
          type="button"
          onClick={onToggleTheme}
          title={theme === "dark" ? "Switch to light theme (⌘D)" : "Switch to dark theme (⌘D)"}
          className="flex size-7 items-center justify-center rounded-md text-faint transition-colors hover:bg-hover hover:text-text"
        >
          {theme === "dark" ? <Sun className="size-3.5" /> : <Moon className="size-3.5" />}
        </button>
      </div>

      {/* Command palette trigger */}
      <div className="px-3 pb-3">
        <button
          type="button"
          onClick={onOpenPalette}
          className="flex h-8 w-full items-center gap-2 rounded-md border border-border bg-panel px-2.5 text-[12px] text-faint transition-colors hover:border-border-strong hover:text-muted"
        >
          <Command className="size-3.5" />
          <span className="flex-1 truncate text-left text-[11px] whitespace-nowrap">Search commands…</span>
          <kbd className="rounded border border-border bg-elevated px-1.5 py-0.5 font-mono text-[10px] text-muted">
            ⌘K
          </kbd>
        </button>
      </div>

      {/* Navigation */}
      <nav className="flex-1 space-y-4 overflow-y-auto px-3 pb-4">
        {NAV.map((item) => {
          const showGroup = item.group !== lastGroup;
          lastGroup = item.group;
          return (
            <div key={item.id}>
              {showGroup && (
                <p className="mb-1.5 px-2.5 text-[10px] font-semibold uppercase tracking-[0.09em] text-faint">
                  {item.group}
                </p>
              )}
              <div className="space-y-0.5">
                <NavItem
                  active={page === item.id}
                  icon={item.icon}
                  label={item.label}
                  keys={item.keys}
                  dot={item.id === "updates" && updateAvailable}
                  onClick={() => onNavigate(item.id)}
                />
              </div>
            </div>
          );
        })}
      </nav>

      {/* Protection status card */}
      <div className="border-t border-border p-3">
        <div className="mb-2 rounded-lg border border-border bg-panel p-3">
          <div className="flex items-center justify-between">
            <span className="flex items-center gap-1.5 text-[11px] font-medium text-muted">
              <ShieldCheck className={cn("size-3.5", armed ? "text-success" : "text-faint")} />
              Protection
            </span>
            <span
              className={cn(
                "size-1.5 rounded-full",
                armed ? "bg-success animate-pulse-dot" : "bg-faint",
              )}
            />
          </div>
          <p className="mt-1.5 text-[12px] font-medium text-text">
            {armed ? `${protectedCount} active rule match${protectedCount === 1 ? "" : "es"}` : "Currently disarmed"}
          </p>
          <p className="mt-0.5 text-[11px] text-faint">
            {live ? `${backend?.name} driver active` : "Preview / simulation mode"}
          </p>
          <button
            type="button"
            onClick={onToggleArm}
            title={armed ? "Pause protection and automation; keep saved rules" : "Turn on protection and apply saved rules"}
            aria-pressed={armed}
            className={cn(
              "mt-2.5 h-8 w-full rounded-md text-[12px] font-medium transition-all",
              armed
                ? "border border-border-strong bg-elevated text-text hover:bg-hover"
                : "bg-accent text-white shadow-sm hover:bg-accent-hover",
            )}
          >
            {armed ? "Disarm protection" : "Arm protection"}
          </button>
        </div>

        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={onOpenPalette}
            title="Keyboard shortcuts"
            className="flex h-7 flex-1 items-center justify-center gap-1.5 rounded-md text-[11px] text-muted transition-colors hover:bg-hover hover:text-text"
          >
            <CircleHelp className="size-3.5" />
            Shortcuts
          </button>
        </div>

        <div className="mt-2 flex items-center justify-center gap-1.5 text-[10px] text-faint">
          <Activity className="size-3" />
          No telemetry · signed updates
        </div>
      </div>
    </aside>
  );
}
