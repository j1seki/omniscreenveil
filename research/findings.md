# Cross-platform Cloakly alternative: project research and implementation plan

The goal is to build an open-source application that provides Cloakly's core functionality on Windows, macOS and Linux, with one interface and native integrations for each operating system.

I'll separate this into two parts:

1. The product we discussed: requirements, behavior and architecture.
2. Existing projects: how they work, how they differ, what we can reuse, and the recommended starting point.

# Part 1 — The product we discussed

## 1. Product definition

A desktop privacy utility that lets users select individual applications or windows and prevent their contents from appearing in supported screen captures.

[Cloakly - Free download and install on Windows | Microsoft Store](https://images.openai.com/static-rsc-4/uK2dNzn0zFyg9rlxRwXlEZJdd7Zw7DNfbmTMZehLfiPXvxRPO4gb0sM5DKrf3IzNMO5DWL-DDhxTyBRC-CXwj5CYK0vPhAU66dEoYKcNCofL7w51DIymxkexpLbMCJYmfnBKFtHJtRJSs-hQ6WzqrU7fU7aUR4i3WaeLXH0oqS4?purpose=inline)

[apps.microsoft.com](http://apps.microsoft.com)

Example workflow:

1. Open the utility.
2. Select Chrome, WhatsApp, a password manager, or another application.
3. Enable protection.
4. Share the entire desktop through Discord, Zoom or OBS.
5. The audience cannot see the selected window.
6. The user can still see, interact with and use the window normally.

The key distinction is that this is not an application minimizer, window hider or virtual desktop manager. It is a selective screen-capture privacy layer.

## 2. Functional requirements


| Feature               | Priority | Description                                        |
| --------------------- | -------- | -------------------------------------------------- |
| Application discovery | P0       | Detect currently running applications              |
| Window selection      | P0       | Select individual windows                          |
| App-wide selection    | P0       | Protect all windows belonging to an app            |
| Capture exclusion     | P0       | Hide selected windows from supported capture paths |
| Local visibility      | P0       | Keep protected windows visible to the owner        |
| Toggle protection     | P0       | Enable/disable without closing applications        |
| Global shortcut       | P1       | Toggle protection while sharing                    |
| Persistent rules      | P1       | Remember selected applications                     |
| Automatic protection  | P1       | Detect sharing sessions and activate rules         |
| Multi-monitor         | P1       | Protection follows windows across monitors         |
| Transparency mode     | P2       | Make selected windows transparent to the audience  |
| Tray/menu-bar control | P1       | Manage protection without opening the main UI      |
| Capture verification  | P0       | Verify that protected content is actually excluded |


P0 means essential to the initial usable product.

## 3. Supported applications

The first target should be capture applications, not specific meeting services.


| Application     | Target                                      |
| --------------- | ------------------------------------------- |
| Discord         | Screen sharing                              |
| Zoom            | Screen sharing                              |
| OBS Studio      | Display/window capture                      |
| Microsoft Teams | Screen sharing                              |
| Google Meet     | Browser screen sharing                      |
| Twitch          | Via OBS or another broadcasting application |


Cloakly advertises compatibility with these services, but its implementation is Windows-specific.

Cloakly+2



## 4. Technical architecture

I recommend a native-first architecture rather than trying to implement all operating systems through a single desktop framework.

Shared desktop application

Tauri 2 + React

Application list · Window selection · Settings · Hotkeys

Shared Rust core

Rules · Window registry · State · IPC · Logging

Windows

Win32 APIs

macOS

AppKit / Quartz

Linux

X11 / Wayland

Capture verification

Test actual captured pixels, not just API return values

### Important OS limitations


| OS            | Native capability                                  | Limitation                                                                                                            |
| ------------- | -------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| Windows       | `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` | Generally applies to windows owned by the calling process; Cloakly describes additional window-interaction techniques |
| macOS         | `NSWindow.sharingType = .none`                     | Can be set by the owning app, not a general controller of arbitrary third-party windows                               |
| Linux X11     | XACE server hooks                                  | Requires an Xorg-level extension and privileged integration                                                           |
| Linux Wayland | Compositor-controlled capture                      | No universal third-party window-exclusion mechanism                                                                   |


This is the most important architectural constraint: we should not promise identical functionality simply because the frontend is cross-platform.

# Part 2 — Existing projects and how they differ

This is where the research becomes important. I found projects that solve different parts of the problem, but none that I could verify as a complete, ready-to-use Cloakly alternative across all three desktop operating systems.

There are three fundamentally different techniques:


| Technique                | How it works                                     | What the audience sees             | Local experience                 |
| ------------------------ | ------------------------------------------------ | ---------------------------------- | -------------------------------- |
| Native capture exclusion | OS compositor excludes a window                  | Window disappears or becomes blank | Window remains visible           |
| Window hiding            | Hide/minimize the actual application window      | Window disappears                  | Window also disappears locally   |
| Filtered sharing         | Capture a modified desktop and share that output | Filtered image                     | Original desktop remains visible |


This distinction matters more than the choice of programming language.

## 1. Windows projects

Most mature implementation

### Evanesco

[GitHub — k4yt3x/Evanesco](https://github.com/k4yt3x/Evanesco?utm_source=chatgpt.com)

- License: GPL-3.0.
- Language: Native Windows application with injected DLL.
- Technique: `SetWindowDisplayAffinity` with `WDA_EXCLUDEFROMCAPTURE`.
- Target: Individual windows or entire processes.
- Interface: GUI, process/window selection, automatic hiding rules and system tray.

How it works:

1. Enumerates running processes and their windows.
2. Identifies the selected window.
3. Injects a DLL into the process owning that window.
4. Calls `SetWindowDisplayAffinity` from inside the target process.
5. Windows Desktop Window Manager excludes the window from supported capture operations.
6. The application remains visible to the user.

It uses DLL injection because Windows restricts this API to the process that owns the target window.

GitHub+1



Assessment: Excellent technical reference. This is the implementation I would study first for the Windows backend.

### hide-window

[GitHub — Shaku-Med/hide-window](https://github.com/Shaku-Med/hide-window?utm_source=chatgpt.com)

This is particularly interesting because it documents the technical limitations clearly.

- Python GUI.
- Individual window selection.
- Application grouping.
- Hide/show controls.
- Keep-window-active functionality.
- Uses the same DLL injection approach.

Important: Despite its name and cross-platform discussion, the actual working implementation is Windows-specific. Its macOS backend is read-only and cannot apply capture protection to other applications' windows.

GitHub+1



Assessment: Good reference for UI behavior, window discovery, grouping and the protection state model. Not a cross-platform codebase to adopt.

### Windows comparison


| Capability               | Evanesco             | hide-window         | Cloakly                                    |
| ------------------------ | -------------------- | ------------------- | ------------------------------------------ |
| Select windows           | Yes                  | Yes                 | Yes                                        |
| Select processes/apps    | Yes                  | Yes                 | Yes                                        |
| Actual capture exclusion | Yes                  | Yes                 | Yes                                        |
| DLL injection            | Yes                  | Yes                 | Its privacy policy describes DLL injection |
| Automatic rules          | Yes                  | Yes                 | Yes                                        |
| Keep target app active   | Not its main feature | Yes                 | Not advertised                             |
| Open source              | GPL-3.0              | No license verified | Proprietary                                |


Cloakly's own technical documentation confirms its use of `WDA_EXCLUDEFROMCAPTURE`.

Cloakly Help+2



## 2. macOS projects

The most interesting research area

### Mica

[GitHub — Vedant-29/mica](https://github.com/Vedant-29/mica?utm_source=chatgpt.com)

[Screen Sharing Got Smarter (and More Private) on macOS: Understanding the System Private Window Picker](https://images.openai.com/static-rsc-4/VWs-Ry1QVdNqcsqdXMsAkktkwEWSYPx7_y2fZm7oWsO9wINu5s50sVLtzbZSfNLuNCAQtGCQFVvOS2FxyiVzfladh5zUxAcgz_T3GAF9atapX-4xdZKCLbI_lh5aU4db8cs2LuK2KqUcUDG9Bf9g2tt4NKWRYDZTnzYySm9RZYQ?purpose=inline)

[blog.addpipe.com](http://blog.addpipe.com)

- License: MIT.
- Language: Swift.
- Minimum OS: macOS 15.
- Interface: Menu-bar application.
- Automatic detection: Yes.

Features:

- Hide all windows.
- Hide all except the frontmost window.
- Hide only selected applications.
- Hide everything except selected applications.
- Hide Dock, wallpaper, desktop icons and menu-bar icons.
- Detect screen sharing and recording.
- Restore previous desktop state.
- Crash recovery.

Mica's implementation is different from Cloakly's.

It hides actual desktop windows and other desktop elements, rather than providing a general-purpose equivalent of Windows `WDA_EXCLUDEFROMCAPTURE` for arbitrary third-party windows. Consequently, it does not meet the exact requirement of leaving the protected app visible and usable locally.

It also relies on some private macOS APIs for desktop and capture detection.

GitHub+1



What we should reuse:

- Screen-sharing detection.
- Trigger and exclusion rules.
- Menu-bar UX.
- Keyboard shortcut handling.
- State preservation and crash recovery.
- App-selection interface.

My assessment: Best macOS project to study for product behavior and automation, but not a direct capture-exclusion backend.

### Cloak

[GitHub — D3OXY/Cloak](https://github.com/D3OXY/Cloak?utm_source=chatgpt.com)

[Cloakly: Hide apps during screen shares – keep your desktop private | Product Hunt](https://images.openai.com/static-rsc-4/XTZJGgfIKsnqFvJxiG3WeR3oUENAv5UMHN76pwVvoOFd7PxjlwJZHxMnvA8ux32P17JacVS1TQRfH5k838iPHbEExkQAbLHhPjcsY0Cos9KwEdiIHaft6N2nkpsQppve9E8WD-8GWxeuWiG6GM-2DMOog1RHM5tF-BlLUB-g-M8?purpose=inline)

[producthunt.com](http://producthunt.com)

This is a different approach.

Instead of changing the capture properties of your existing windows, it creates a separate window that you share.

Features:

- Global privacy hotkey.
- Shareable window.
- Privacy placeholder.
- Blur, image and black-screen modes.
- App exclusion.
- Hides its own controls from capture.

How it works:

```
Your actual desktop
       |
       v
  Cloak application
       |
       v
  Filtered shareable window
       |
       v
   Zoom / OBS
```

The user shares the Cloak window rather than the actual desktop.

That means it is not transparent to existing screen-sharing workflows: you must select the special window to share.

GitHub+1



Assessment: Excellent fallback implementation to study. Check its repository license before reusing code; do not assume the source is permissively licensed just because it is public.

### Clave

[GitHub — CITGuru/clave](https://github.com/CITGuru/clave?utm_source=chatgpt.com)

This is an especially useful technical reference, rather than a proven finished alternative.

Its screen-capture protection documentation compares:

- Windows display affinity.
- macOS window-sharing restrictions.
- Process injection.
- Window lifecycle tracking.
- Capture verification.
- Exclusion versus watermarking.

Its macOS analysis identifies the fundamental problem: you cannot simply set `NSWindow.sharingType = .none` on an arbitrary window belonging to another application.

GitHub+1



Assessment: Study its architecture and technical documentation before implementing the native backends. Do not mistake a documented design for an already working cross-platform product.

### macOS comparison


| Capability                                     | Mica                          | D3OXY Cloak                   | Clave                     |
| ---------------------------------------------- | ----------------------------- | ----------------------------- | ------------------------- |
| Select apps                                    | Yes                           | Yes, for filtered sharing     | Design support            |
| Auto-detect sharing                            | Yes                           | Not its primary approach      | Research                  |
| Original app remains visible locally           | No, when its window is hidden | Yes                           | Target design             |
| Exclude arbitrary third-party windows natively | No                            | Not required for its approach | Identifies limitation     |
| Share filtered output                          | No                            | Yes                           | Not its main purpose      |
| License                                        | MIT                           | Verify before reuse           | Verify repository license |


Conclusion: Mica and Cloak are complementary, not competing implementations of the same technique.

## 3. Linux projects

Most fragmented platform

Linux requires separate consideration of X11 and Wayland.

### X11WithPrivacy

[GitHub — Marksonthegamer/X11WithPrivacy](https://github.com/Marksonthegamer/X11WithPrivacy?utm_source=chatgpt.com)

[This Week in Plasma: Background Apps and Zoom Up-Scaling - KDE Blogs](https://images.openai.com/static-rsc-4/A-n_i3KnGLxZH-EAP4Y3-9ARXeu6heX-ZlhzednACRWDfBqwJ8GXeYRTjJwrCNax4BbkFJAL8JUPtIcFWHhC9eHm4bRGJVjDyj9taqlq--x8IPBhBEICdzxrBA3HI5k5E_ErTDQfAe62sAKVyJOHu1o2OAl-2eUfOAbfPv7s37w?purpose=inline)

[blogs.kde.org](http://blogs.kde.org)

This is one of the most interesting Linux projects I found.

It is an Xorg server extension, not an ordinary desktop utility.

Features:

- Screenshot restriction.
- Screencast restriction.
- XRecord/XTest restrictions.
- Clipboard access control.
- D-Bus integration.
- Policy-based access control.

How it works:

It uses XACE hooks inside the Xorg server to intercept requests such as:

- `XGetImage`
- `MIT-SHM:GetImage`
- `CopyArea`
- XRecord
- XTest

It can distinguish between a process accessing its own windows and a process trying to capture another process's windows.

GitHub+1



Important difference: This is primarily a security mechanism that controls which processes can capture desktop content. It does not implement Cloakly's straightforward selected-window exclusion behavior.

Also, it does not work on Wayland.

Assessment: Study its server-level approach, but I would not make this our primary product architecture.

### [X.org](http://X.org) Security Module

[GitHub — ultract/](https://github.com/ultract/X.org-Security-Module?utm_source=chatgpt.com)[X.org](http://X.org)[-Security-Module](https://github.com/ultract/X.org-Security-Module?utm_source=chatgpt.com)

The original project on which X11WithPrivacy is based.

- Xorg extension.
- Screenshot and screencast restrictions.
- Clipboard restrictions.
- D-Bus policy configuration.
- MIT license.

This is useful for understanding the underlying XACE implementation.

GitHub+1



### Wayland Share Bridge

[GitHub — pingod/wayland-share-bridge](https://github.com/pingod/wayland-share-bridge?utm_source=chatgpt.com)

[Nova versão do compositor niri simplifica o uso de apps X11 e adiciona suporte a leitor de tela | SempreUpdate](https://images.openai.com/static-rsc-4/z9sXK7NmSYsi3XsCWEAHgBStC4HlCAM6saCAiuQ6F9xmV9JtSSBWCZ7kYah1uDsojfHucB0HyUHvw1h_dhjoBhKnd8qPc7Ja32hde67E9a_bXOmWxm7wD1ricih0zFXA3iKLQpUAtKW911_1a484rZ1CXFonWsMpyaLJEztg3Zs?purpose=inline)

[sempreupdate.com.br](http://sempreupdate.com.br)

This project is not a capture-exclusion tool, but it is useful for a possible fallback.

It:

1. Captures a Wayland screen or window.
2. Uses compositor protocols or PipeWire through the desktop portal.
3. Mirrors the selected content into an X11 window.
4. Lets applications such as Zoom or OBS capture that mirrored window.

It supports a direct capture mode for compatible compositors, including Hyprland 0.56+, and a portal fallback.

Assessment: A useful example of a filtered-sharing architecture, but it does not hide arbitrary windows from normal desktop capture. Its license is EUPL-1.2.

GitHub



### Linux comparison


| Capability                                 | X11WithPrivacy        | [X.org](http://X.org) Security Module | Wayland Share Bridge           |
| ------------------------------------------ | --------------------- | ------------------------------------- | ------------------------------ |
| X11 support                                | Yes                   | Yes                                   | X11 output                     |
| Native Wayland support                     | No                    | No                                    | Yes, with supported compositor |
| Selective app protection                   | Process access policy | Process access policy                 | Select share source            |
| Hide selected window from all capture apps | No guarantee          | No guarantee                          | No                             |
| Keep original app visible                  | Yes                   | Yes                                   | Yes                            |
| Requires special integration               | Xorg module           | Xorg module                           | Compositor/portal              |
| License                                    | Verify fork           | MIT                                   | EUPL-1.2                       |


Conclusion: There is no equivalent of Evanesco that I could verify for generic Wayland desktops.

# Part 3 — Overall comparison

This is the consolidated comparison of the projects worth investigating.


| Project                               | Platform                | Main technique                    | Closest to our goal? | Reuse recommendation           |
| ------------------------------------- | ----------------------- | --------------------------------- | -------------------- | ------------------------------ |
| Evanesco                              | Windows                 | DLL injection + display affinity  | <br>Yes<br>          | Study backend                  |
| hide-window                           | Windows primarily       | DLL injection + display affinity  | <br>Yes<br>          | Study UI and window management |
| Mica                                  | macOS                   | Window hiding + sharing detection | <br>Partial<br>      | Reuse MIT-licensed components  |
| Cloak                                 | macOS                   | Filtered shareable window         | <br>Alternative<br>  | Study fallback                 |
| Clave                                 | Cross-platform research | Native API analysis               | <br>Very useful<br>  | Study architecture             |
| X11WithPrivacy                        | Linux X11               | XACE server hooks                 | <br>Partial<br>      | Study backend                  |
| [X.org](http://X.org) Security Module | Linux X11               | XACE server hooks                 | <br>Partial<br>      | Study original code            |
| Wayland Share Bridge                  | Linux Wayland           | PipeWire/compositor mirroring     | <br>Alternative<br>  | Study fallback                 |


## Part 4 — My recommended starting point

I would not fork one of these projects and try to turn it into the entire product.

Instead, build one application with independent OS backends.

### Recommended starting repositories

1. Primary Windows reference

Evanesco

[Inspect source code](https://github.com/k4yt3x/Evanesco?utm_source=chatgpt.com)

Learn its DLL injection, window selection and display-affinity implementation.

2. Primary macOS reference

Mica

[Inspect source code](https://github.com/Vedant-29/mica?utm_source=chatgpt.com)

Study its sharing detection, triggers, menu-bar controls and state restoration. Reuse MIT-licensed components where appropriate, but implement the actual protection separately.

3. Linux X11 reference

X11WithPrivacy + original [X.org](http://X.org) Security Module

[Inspect X11WithPrivacy](https://github.com/Marksonthegamer/X11WithPrivacy?utm_source=chatgpt.com) · [Inspect original module](https://github.com/ultract/X.org-Security-Module?utm_source=chatgpt.com)

Investigate XACE and whether selective process policies can be adapted to the product.

4. Linux Wayland reference

Wayland Share Bridge

[Inspect source code](https://github.com/pingod/wayland-share-bridge?utm_source=chatgpt.com)

Use as a reference for compositor capture, window discovery and filtered-sharing fallback.

5. Cross-platform technical reference

Clave

[Inspect documentation](https://github.com/CITGuru/clave?utm_source=chatgpt.com)

Study its platform-by-platform analysis before committing to the backend architecture.

## Part 5 — Architecture I would implement


| Layer         | Technology                  | Responsibility                                                          |
| ------------- | --------------------------- | ----------------------------------------------------------------------- |
| UI            | Tauri 2 + React             | App selection, controls and settings                                    |
| Core          | Rust                        | Rules, state, window registry and IPC                                   |
| Windows       | Rust + Win32                | Display affinity and target-process integration                         |
| macOS         | Swift + AppKit              | Window discovery, sharing detection and available protection mechanisms |
| Linux X11     | C/Rust + XACE               | Server-side capture restrictions                                        |
| Linux Wayland | Rust + compositor protocols | Compositor-specific integrations and fallback                           |
| Testing       | Native capture harnesses    | Verify actual screenshots and streams                                   |


### A critical design decision

I would define three protection states rather than just an on/off toggle:

- Protected: Tested successfully against supported capture methods.
- Partial: Protection works only for certain capture methods or compositors.
- Unsupported: The OS cannot provide the requested protection.

This is especially important for macOS and Linux.

## Part 6 — Development order

I recommend this order:


| Phase | Work                                          | Deliverable                                          |
| ----- | --------------------------------------------- | ---------------------------------------------------- |
| 1     | Native API proof-of-concept on all three OSes | Technical feasibility report                         |
| 2     | Windows backend                               | Working Cloakly-like functionality                   |
| 3     | macOS backend                                 | Mica-inspired controls and best available protection |
| 4     | Linux X11 backend                             | XACE-based implementation                            |
| 5     | Linux Wayland backend                         | Compositor-specific support                          |
| 6     | Unified Tauri application                     | One cross-platform interface                         |
| 7     | Capture testing                               | Discord, Zoom, OBS and browser testing               |
| 8     | Packaging                                     | Windows, macOS and Linux releases                    |


Do not build the full UI before validating the hardest native APIs. Otherwise, you risk spending time polishing a product whose core feature cannot work on a particular OS.

# Final recommendation

Recommended direction

Build a new project rather than forking an existing one.

The strongest combination is:

- Windows: Evanesco's technical approach.
- macOS: Mica's UX and automation, with a separate capture-protection strategy.
- Linux X11: X11WithPrivacy research.
- Linux Wayland: Compositor-specific integration and filtered-sharing fallback.
- Shared architecture: Tauri + Rust.

There is one limitation I would accept from the beginning: macOS and Linux may initially offer fewer guarantees than Windows. Apple documents application-level capture filtering through ScreenCaptureKit, but that filters a stream created by your own app, not arbitrary third-party sharing sessions.

Apple Developer Documentation+1



The real opportunity: Build the first polished, open-source, unified desktop privacy utility that makes the best available native protection accessible through one simple interface.

The product should prioritize preserving the user's normal workflow, not force them to change how they share their desktop. Filtered sharing should be a fallback, not the primary experience.