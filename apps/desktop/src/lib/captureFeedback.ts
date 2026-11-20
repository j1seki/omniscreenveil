import { useLayoutEffect } from "react";
import * as api from "../api";
import type { Rect } from "../types";

export function useCaptureFeedback(windows: boolean) {
  useLayoutEffect(() => {
    const dashboard = document.querySelector("main");
    if (!dashboard || !windows || !api.IN_TAURI) return;

    let last = "";
    let scheduled = 0;
    const update = () => {
      scheduled = 0;
      const scale = window.devicePixelRatio;
      const viewport = dashboard.getBoundingClientRect();
      const areas: Rect[] = [];
      for (const node of dashboard.querySelectorAll("[data-capture-preview]")) {
        const bounds = node.getBoundingClientRect();
        const x = Math.floor(Math.max(bounds.left - 2, viewport.left, 0) * scale);
        const y = Math.floor(Math.max(bounds.top - 2, viewport.top, 0) * scale);
        const right = Math.ceil(Math.min(bounds.right + 2, viewport.right, innerWidth) * scale);
        const bottom = Math.ceil(Math.min(bounds.bottom + 2, viewport.bottom, innerHeight) * scale);
        if (right > x && bottom > y) areas.push({ x, y, w: right - x, h: bottom - y });
      }
      const signature = JSON.stringify(areas);
      if (signature === last) return;
      last = signature;
      void api.setFeedbackAreas(areas).catch((err) => {
        last = "";
        api.recordLog("warn", `Could not publish capture preview geometry: ${err}`);
      });
    };
    const schedule = () => { if (!scheduled) scheduled = requestAnimationFrame(update); };
    const sizes = new ResizeObserver(schedule);
    const observe = () => {
      sizes.disconnect();
      sizes.observe(dashboard);
      for (const node of dashboard.querySelectorAll("[data-capture-preview]")) sizes.observe(node);
      schedule();
    };
    const content = new MutationObserver((entries) => {
      if (entries.some((entry) => [...entry.addedNodes, ...entry.removedNodes].some((node) => node instanceof Element && (node.matches("[data-capture-preview]") || node.querySelector("[data-capture-preview]"))))) observe();
    });
    content.observe(dashboard, { childList: true, subtree: true });
    window.addEventListener("scroll", schedule, true);
    window.addEventListener("resize", schedule);
    dashboard.addEventListener("animationend", schedule);
    observe();
    return () => {
      sizes.disconnect();
      content.disconnect();
      window.removeEventListener("scroll", schedule, true);
      window.removeEventListener("resize", schedule);
      dashboard.removeEventListener("animationend", schedule);
      cancelAnimationFrame(scheduled);
    };
  }, [windows]);
}
