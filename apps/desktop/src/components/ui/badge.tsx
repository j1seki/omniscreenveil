import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "../../lib/utils";

const badgeVariants = cva(
  "inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] font-medium leading-none",
  {
    variants: {
      variant: {
        default: "border-line bg-row text-ink-dim",
        live: "border-good/30 bg-good/10 text-good",
        sim: "border-warn/30 bg-warn/10 text-warn",
        error: "border-bad/30 bg-bad/10 text-bad",
        accent: "border-accent/30 bg-accent/10 text-accent",
        good: "border-good/30 bg-good/10 text-good",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  },
);

export interface BadgeProps
  extends React.HTMLAttributes<HTMLSpanElement>,
    VariantProps<typeof badgeVariants> {}

export function Badge({ className, variant, ...props }: BadgeProps) {
  return <span className={cn(badgeVariants({ variant }), className)} {...props} />;
}

export { badgeVariants };
