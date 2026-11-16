import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "../../lib/utils";

const badgeVariants = cva(
  "inline-flex items-center gap-1 rounded-md border px-1.5 py-0.5 text-[10px] font-medium leading-none",
  {
    variants: {
      variant: {
        default: "border-border bg-elevated text-muted",
        live: "border-success/25 bg-success-soft text-success",
        sim: "border-warning/25 bg-warning-soft text-warning",
        error: "border-danger/25 bg-danger-soft text-danger",
        accent: "border-accent/25 bg-accent-soft text-accent",
        good: "border-success/25 bg-success-soft text-success",
      },
    },
    defaultVariants: { variant: "default" },
  },
);

export interface BadgeProps
  extends React.HTMLAttributes<HTMLSpanElement>,
    VariantProps<typeof badgeVariants> {}

export function Badge({ className, variant, ...props }: BadgeProps) {
  return <span className={cn(badgeVariants({ variant }), className)} {...props} />;
}

export { badgeVariants };
