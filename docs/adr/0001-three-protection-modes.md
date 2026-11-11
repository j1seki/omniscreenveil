# ADR 0001 — Three protection modes instead of one "cloak" switch

- Status: accepted
- Date: 2026-11

## Context

Cloakly-style products present a single "protect this app" switch. The
underlying techniques are not equivalent: Windows can remove a window from
capture while keeping it usable; macOS and Linux cannot do that with public
APIs. A single switch would either lie on two platforms or be unavailable on
one, and users would not know which promise they got.

## Decision

Expose three explicit modes, each with its own promise and cost:

- `Exclusion` — visible locally, absent from capture (Windows only).
- `Surface` — share a composited window with protected regions masked
  (everywhere; partial on Wayland).
- `StrictHide` — hidden locally as well (everywhere, with permission).

The capability matrix is data, not UI copy, and the engine downgrades an
unsupported request to the best supported lower-precedence mode while reporting
the effective mode.

## Consequences

- The UI must show the matrix and the effective mode, not just a toggle.
- The engine needs reconciliation between requested and effective modes.
- Users on macOS/X11/Wayland see `Exclusion` unsupported immediately, which is
  the honest outcome.
- Documentation and marketing must not collapse the modes into "works
  everywhere".
