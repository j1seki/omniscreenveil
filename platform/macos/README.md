# macOS Phase 0 helpers (`VeilMac`)

Public API only. This package exists so the macOS backend is validated on a
real machine instead of being claimed from a Linux VM.

## What it proves

| Command | API | Claim |
|---|---|---|
| `list` | `CGWindowListCopyWindowInfo` | real window inventory + geometry, no permissions |
| `hide` / `show` | Accessibility `AXMinimized` | strict-hide works on foreign windows with consent |
| `capture <display>` | ScreenCaptureKit `SCScreenshotManager` | capture path for the Veil Surface and verifier |
| `permission` | `AXIsProcessTrustedWithOptions` | clear, prompt-driven consent flow |
| `selftest` | all of the above | invariants (layer 0 only, own process excluded, sane geometry) |

## What it deliberately does **not** do

macOS has no public API to remove a window from another capture client's
stream. `Exclusion` stays `unsupported` in the capability matrix. The Veil
Surface filters selected window IDs out of its own ScreenCaptureKit capture.

## Build and run

```sh
cd platform/macos
swift build -c release
.build/release/veilmac-spike permission
.build/release/veilmac-spike list
.build/release/veilmac-spike capture 0      # main display; writes veilmac-capture.ppm
# Add one or more IDs from `list` to filter them in this capture only.
swift test
```

`capture` needs Screen Recording consent the first time. `hide`/`show` need
Accessibility consent. Both prompts come from macOS itself, and the app keeps
working in Surface-only mode if the user declines.

## Linking from Rust

`VeilMac` is a static library with a tiny C ABI (`Sources/VeilMac/Exports.swift`):

```c
int32_t veil_macos_hide_window(uint64_t window);
int32_t veil_macos_show_window(uint64_t window);
int32_t veil_macos_accessibility_trusted(void);
int32_t veil_macos_request_accessibility(void);
int32_t veil_macos_screen_recording_granted(void);
int32_t veil_macos_request_screen_recording(void);
char   *veil_macos_last_error(void);          /* free with veil_macos_free_string */
uint8_t *veil_macos_capture_display(uint32_t display, const uint32_t *excluded_ids,
                                   size_t excluded_count, uint32_t *w, uint32_t *h);
void    veil_macos_free_buffer(uint8_t *buffer);
```

Build the release static library and set `VEIL_MACOS_STATIC_LIB` to its absolute
path if you aren't using the default `.build/release/libVeilMac.a` location.
The library links into the macOS binary; it is not a separate bundled helper.
For distributing the desktop app, see the [macOS release signing instructions](../../README.md#macos-release-signing).
