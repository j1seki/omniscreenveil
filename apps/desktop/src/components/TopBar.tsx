import { Activity, Radio } from "lucide-react";
import type { BackendInfo, Platform } from "../types";
import { PLATFORM_LABELS } from "../types";
import { Badge } from "./ui/badge";
import { ShieldMark } from "./icons";

export function TopBar({
  backend,
  platform,
}: {
  backend: BackendInfo | null;
  platform: Platform | null;
}) {
  return (
    <header className="flex items-center justify-between gap-4 px-5 py-3">
      <div className="flex items-center gap-3">
        <span className="flex size-9 items-center justify-center rounded-lg border border-line bg-panel">
          <ShieldMark size={20} />
        </span>
        <div>
          <h1 className="text-[15px] font-semibold leading-tight tracking-tight">
            OmniScreenVeil
          </h1>
          <p className="text-[11px] leading-tight text-ink-faint">
            Selective screen-capture privacy
          </p>
        </div>
      </div>

      <div className="flex items-center gap-2">
        {platform && (
          <Badge variant="default">
            <Activity className="size-3" />
            {PLATFORM_LABELS[platform]}
          </Badge>
        )}
        {backend && (
          <Badge variant={backend.live ? "live" : "sim"}>
            <Radio className="size-3" />
            {backend.live ? "Live backend" : "Preview data"}
          </Badge>
        )}
      </div>
    </header>
  );
}
