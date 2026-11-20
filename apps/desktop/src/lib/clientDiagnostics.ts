import { recordLog } from "../api";

export function captureClientErrors() {
  window.addEventListener("error", (event) => {
    if (event.message) recordLog("error", event.message);
  });
  window.addEventListener("unhandledrejection", (event) => {
    recordLog("error", String(event.reason));
  });
}
