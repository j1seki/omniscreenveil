import { cn } from "../lib/utils";

export function ShieldMark({ size = 22, className }: { size?: number; className?: string }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      aria-hidden="true"
      className={cn("text-accent", className)}
    >
      <path
        d="M12 2.5 20 6v6.2c0 4.6-3.2 8.1-8 9.3-4.8-1.2-8-4.7-8-9.3V6l8-3.5Z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinejoin="round"
      />
      <path
        d="M8.4 12.2 11 14.8l4.8-5"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function AppGlyph({ name, size = 18 }: { name: string; size?: number }) {
  const initial = (name.trim()[0] ?? "?").toUpperCase();
  let hash = 0;
  for (let i = 0; i < name.length; i++) hash = (hash * 31 + name.charCodeAt(i)) % 360;
  return (
    <span
      className="inline-flex shrink-0 items-center justify-center rounded-md font-semibold text-white/90"
      style={{
        width: size,
        height: size,
        fontSize: size * 0.5,
        background: `linear-gradient(135deg, hsl(${hash} 45% 32%), hsl(${(hash + 40) % 360} 40% 22%))`,
      }}
      aria-hidden="true"
    >
      {initial}
    </span>
  );
}
