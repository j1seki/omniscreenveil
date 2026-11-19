import AppKit
import ApplicationServices

private final class WindowEvents {
    static let shared = WindowEvents()

    private var observers: [pid_t: AXObserver] = [:]
    private var tokens: [NSObjectProtocol] = []
    private var notify: (@convention(c) () -> Void)?

    func start(_ callback: @escaping @convention(c) () -> Void) {
        guard tokens.isEmpty else { return }
        notify = callback
        let workspace = NSWorkspace.shared
        let events: [Notification.Name] = [NSWorkspace.didLaunchApplicationNotification,
            NSWorkspace.didTerminateApplicationNotification, NSWorkspace.didActivateApplicationNotification,
            NSWorkspace.didHideApplicationNotification, NSWorkspace.didUnhideApplicationNotification,
            NSWorkspace.activeSpaceDidChangeNotification]
        for name in events {
            tokens.append(workspace.notificationCenter.addObserver(forName: name, object: nil, queue: .main) {
                [weak self] _ in self?.sync(); self?.notify?()
            })
        }
        sync()
    }

    private func sync() {
        guard AXIsProcessTrusted() else { return }
        let pids = Set(NSWorkspace.shared.runningApplications.filter {
            $0.activationPolicy != .prohibited && $0.processIdentifier != ProcessInfo.processInfo.processIdentifier
        }.map(\.processIdentifier))
        for pid in Array(observers.keys) where !pids.contains(pid) {
            if let observer = observers.removeValue(forKey: pid) {
                CFRunLoopRemoveSource(CFRunLoopGetMain(), AXObserverGetRunLoopSource(observer), .commonModes)
            }
        }
        for pid in pids where observers[pid] == nil {
            var created: AXObserver?
            let result = AXObserverCreate(pid, { _, _, _, context in
                guard let context else { return }
                let watcher = Unmanaged<WindowEvents>.fromOpaque(context).takeUnretainedValue()
                watcher.notify?()
            }, &created)
            guard result == .success, let observer = created else { continue }
            let application = AXUIElementCreateApplication(pid)
            AXUIElementSetMessagingTimeout(application, 0.25)
            let context = Unmanaged.passUnretained(self).toOpaque()
            for event in [kAXWindowCreatedNotification, kAXFocusedWindowChangedNotification,
                          kAXMainWindowChangedNotification,
                          kAXUIElementDestroyedNotification, kAXWindowMiniaturizedNotification,
                          kAXWindowDeminiaturizedNotification, kAXTitleChangedNotification] {
                AXObserverAddNotification(observer, application, event as CFString, context)
            }
            CFRunLoopAddSource(CFRunLoopGetMain(), AXObserverGetRunLoopSource(observer), .commonModes)
            observers[pid] = observer
        }
    }
}

@_cdecl("veil_macos_watch_windows")
public func veilMacosWatchWindows(_ callback: @escaping @convention(c) () -> Void) {
    DispatchQueue.main.async { WindowEvents.shared.start(callback) }
}
