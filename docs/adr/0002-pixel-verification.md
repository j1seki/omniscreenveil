# ADR 0002 — Pixel verification instead of API return codes

- Status: accepted
- Date: 2026-11

## Context

Every capture-blocking API can appear to succeed. Display affinity is ignored by
some capture paths, a compositor filter may cover the wrong rectangle, and an
Accessibility call may return success for a window that a hardened app keeps
drawing. A desktop privacy tool that reports "protected" from a return code is
actively dangerous.

## Decision

The only accepted proof is a pixel diff over a baseline:

1. composite a deterministic 8×8 canary token,
2. capture a baseline through the target path,
3. arm protection,
4. capture again and detect the token.

A missing baseline yields `Inconclusive`, never `Protected`. The verification
record includes the capture path, OS build, timestamp and a human-readable
detail. The UI exposes both detections.

## Consequences

- The product can say "Protected" with evidence, and "Visible" when the OS
  ignored the request.
- Capture must be available on every platform; where it is not (Wayland without
  a portal), the claim is partial or unsupported.
- Tests need real capture paths. The X11 end-to-end test runs against `Xvfb`;
  macOS uses ScreenCaptureKit; Windows uses Windows Graphics Capture.
- The canary must not be mistaken for user content. It is rendered into a
  dedicated window or region and removed after the run.
