// IncomingCallBanner.swift — ClipRelay macOS
// Apple-style incoming call banner overlay.
//
// Architecture mirrors NotificationOverlayWindow.swift:
//   CallBannerWindowManager owns a CallBannerPanel (NSPanel)
//   that hosts a SwiftUI CallBannerView.
//
// The banner appears when store.activeCall transitions from nil → ringing,
// plays a ringtone loop via AVAudioPlayer, and dismisses on accept/decline/idle.

import AppKit
import AVFoundation
import Combine
import SwiftUI

// MARK: - Window Manager

@MainActor
final class CallBannerWindowManager: NSObject {
    private let store: ClipRelayStore
    private let panel: CallBannerPanel
    private let hostingView: CallBannerHostingView<CallBannerContainerView>
    private var audioPlayer: AVAudioPlayer?
    private var ringRepeatTimer: Timer?
    private var cancellables = Set<AnyCancellable>()

    init(store: ClipRelayStore) {
        self.store = store
        self.panel = CallBannerPanel()
        self.hostingView = CallBannerHostingView(rootView: CallBannerContainerView(store: store))
        super.init()

        panel.contentView = hostingView

        NotificationCenter.default.addObserver(
            self,
            selector: #selector(layoutPanel),
            name: NSApplication.didChangeScreenParametersNotification,
            object: nil
        )

        store.$activeCall
            .receive(on: RunLoop.main)
            .removeDuplicates()
            .sink { [weak self] call in
                self?.handleCallUpdate(call)
            }
            .store(in: &cancellables)
    }

    deinit {
        NotificationCenter.default.removeObserver(self)
        audioPlayer?.stop()
    }

    private func handleCallUpdate(_ call: IncomingCallState?) {
        layoutPanel()
        if let call = call, (call.isRinging || call.isOffhook) {
            panel.orderFrontRegardless()
            if call.isRinging {
                startRingtone()
                NSHapticFeedbackManager.defaultPerformer.perform(.levelChange, performanceTime: .default)
            } else {
                stopRingtone()
            }
        } else {
            panel.orderOut(nil)
            stopRingtone()
        }
    }

    @objc private func layoutPanel() {
        guard let screen = activeScreen else { return }
        let visible = screen.visibleFrame
        let width: CGFloat = 380
        let height: CGFloat = 160
        let frame = NSRect(
            x: visible.midX - width / 2,
            y: visible.maxY - height - 12,
            width: width,
            height: height
        )
        panel.setFrame(frame, display: false)
    }

    private var activeScreen: NSScreen? {
        if let key = NSApp.keyWindow?.screen { return key }
        let mouse = NSEvent.mouseLocation
        return NSScreen.screens.first { NSMouseInRect(mouse, $0.frame, false) } ?? NSScreen.main
    }

    // MARK: - Audio

    private func startRingtone() {
        guard audioPlayer == nil || audioPlayer?.isPlaying == false else { return }

        // Try bundled ringtone first
        let bundleURL = Bundle.main.url(forResource: "ringtone", withExtension: "caf")
            ?? Bundle.main.url(forResource: "ringtone", withExtension: "mp3")

        if let url = bundleURL {
            do {
                audioPlayer = try AVAudioPlayer(contentsOf: url)
                audioPlayer?.numberOfLoops = -1
                audioPlayer?.volume = 0.7
                audioPlayer?.play()
                return
            } catch {
                NSLog("ClipRelay: failed to load bundled ringtone: \(error)")
            }
        }

        // Fallback: play a system sound on repeat using a Timer
        let systemSoundNames = ["Glass", "Ping", "Pop", "Tink"]
        let soundName = systemSoundNames.first(where: { NSSound(named: $0) != nil }) ?? "Glass"
        if let sound = NSSound(named: soundName) {
            sound.volume = 0.8
            sound.play()
            // Repeat every 3 seconds via a stored timer reference in the panel's run loop
            ringRepeatTimer?.invalidate()
            ringRepeatTimer = Timer.scheduledTimer(withTimeInterval: 3.0, repeats: true) { _ in
                NSSound(named: soundName)?.play()
            }
        }
    }

    private func stopRingtone() {
        audioPlayer?.stop()
        audioPlayer = nil
        ringRepeatTimer?.invalidate()
        ringRepeatTimer = nil
    }
}

// MARK: - Panel

private final class CallBannerHostingView<Content: View>: NSHostingView<Content> {
    override func hitTest(_ point: NSPoint) -> NSView? {
        let view = super.hitTest(point)
        NSLog("ClipRelay DEBUG: CallBannerHostingView hitTest at point: \(point), returned: \(String(describing: view))")
        return view
    }
}

private final class CallBannerPanel: NSPanel {
    init() {
        super.init(
            contentRect: NSRect(x: 0, y: 0, width: 380, height: 160),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        
        level = .statusBar
        hasShadow = true
        isOpaque = false
        backgroundColor = .clear
        hidesOnDeactivate = false
        ignoresMouseEvents = false
        becomesKeyOnlyIfNeeded = true
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .ignoresCycle]
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }

    override func sendEvent(_ event: NSEvent) {
        if event.type == .leftMouseDown {
            NSLog("ClipRelay DEBUG: Window received leftMouseDown event at location: \(event.locationInWindow)")
        }
        super.sendEvent(event)
    }
}

// MARK: - SwiftUI Container

private struct CallBannerContainerView: View {
    @ObservedObject var store: ClipRelayStore

    var body: some View {
        Group {
            if let call = store.activeCall, (call.isRinging || call.isOffhook) {
                CallBannerView(
                    call: call,
                    onAccept: { store.acceptCall() },
                    onDecline: { store.declineCall() },
                    onRouteAudio: { route in store.routeAudio(to: route) }
                )
                .transition(.move(edge: .top).combined(with: .opacity))
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .padding(8)
        .animation(.crSpring, value: store.activeCall)
    }
}

// MARK: - Call Button Style
private struct CallButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .scaleEffect(configuration.isPressed ? 0.9 : 1.0)
            .opacity(configuration.isPressed ? 0.8 : 1.0)
            .animation(.crSpring, value: configuration.isPressed)
    }
}

// MARK: - Call Banner View

private struct CallBannerView: View {
    let call: IncomingCallState
    let onAccept: () -> Void
    let onDecline: () -> Void
    let onRouteAudio: (String) -> Void

    @State private var ringPulse = false
    @State private var callDuration: TimeInterval = 0
    let timer = Timer.publish(every: 1, on: .main, in: .common).autoconnect()

    @Environment(\.colorScheme) var colorScheme

    // ── Design tokens ────────────────────────────────────────────────────────
    private let acceptGreen = Color(hex: 0x30D158)
    private let declineRed = Color(hex: 0xFF453A)
    private let activeBlue = Color.blue

    private var formatDuration: String {
        let m = Int(callDuration) / 60
        let s = Int(callDuration) % 60
        return String(format: "%02d:%02d", m, s)
    }

    var body: some View {
        HStack(spacing: 16) {
            // ── Caller avatar / phone icon ────────────────────────────────
            ZStack {
                if call.isRinging {
                    Circle()
                        .fill(acceptGreen.opacity(0.15))
                        .frame(width: 56, height: 56)
                        .scaleEffect(ringPulse ? 1.25 : 1.0)
                        .opacity(ringPulse ? 0.0 : 0.4)
                }

                Circle()
                    .fill(colorScheme == .dark ? Color.white.opacity(0.15) : Color.black.opacity(0.1))
                    .frame(width: 48, height: 48)

                Image(systemName: call.isOffhook ? "phone.connection" : "phone.fill")
                    .font(.system(size: 20, weight: .semibold))
                    .foregroundStyle(call.isOffhook ? activeBlue : acceptGreen)
                    .rotationEffect(.degrees((call.isRinging && ringPulse) ? -15 : (call.isRinging ? 15 : 0)))
            }
            .onAppear {
                if call.isRinging {
                    withAnimation(.easeInOut(duration: 0.5).repeatForever(autoreverses: true)) {
                        ringPulse = true
                    }
                }
            }

            // ── Caller info ──────────────────────────────────────────────
            VStack(alignment: .leading, spacing: 4) {
                if call.isOffhook {
                    Text("Ongoing Call • \(formatDuration)")
                        .font(.system(size: 11, weight: .semibold, design: .monospaced))
                        .foregroundStyle(activeBlue)
                        .textCase(.uppercase)
                        .tracking(0.8)
                        .onReceive(timer) { _ in callDuration += 1 }
                } else {
                    Text("Incoming Call")
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(acceptGreen)
                        .textCase(.uppercase)
                        .tracking(0.8)
                }

                Text(call.displayName)
                    .font(.system(size: 18, weight: .bold))
                    .foregroundStyle(Color.primary)
                    .lineLimit(1)

                HStack(spacing: 4) {
                    Image(systemName: "iphone")
                        .font(.system(size: 10))
                    Text(call.deviceName)
                        .font(.system(size: 11))
                }
                .foregroundStyle(Color.secondary)
            }

            Spacer(minLength: 0)

            // ── Action buttons ────────────────────────────────────────────
            if call.isRinging {
                VStack(spacing: 8) {
                    Button(action: {
                        NSLog("ClipRelay DEBUG: Accept button clicked inside CallBannerView!")
                        onAccept()
                    }) {
                        ZStack {
                            Circle().fill(acceptGreen)
                            Image(systemName: "phone.fill")
                                .font(.system(size: 14, weight: .bold))
                                .foregroundStyle(.white)
                        }
                        .frame(width: 44, height: 44)
                        .contentShape(Circle())
                    }
                    .buttonStyle(CallButtonStyle())
                    .shadow(color: acceptGreen.opacity(0.4), radius: 8, y: 2)

                    Button(action: {
                        NSLog("ClipRelay DEBUG: Decline button clicked inside CallBannerView!")
                        onDecline()
                    }) {
                        ZStack {
                            Circle().fill(declineRed)
                            Image(systemName: "phone.down.fill")
                                .font(.system(size: 14, weight: .bold))
                                .foregroundStyle(.white)
                        }
                        .frame(width: 44, height: 44)
                        .contentShape(Circle())
                    }
                    .buttonStyle(CallButtonStyle())
                    .shadow(color: declineRed.opacity(0.4), radius: 8, y: 2)
                }
            } else {
                // Ongoing call actions
                HStack(spacing: 12) {
                    // Audio Routing Menu
                    Menu {
                        Button(action: { onRouteAudio("earpiece") }) {
                            Label("Phone Earpiece", systemImage: "iphone")
                        }
                        Button(action: { onRouteAudio("speaker") }) {
                            Label("Speakerphone", systemImage: "speaker.wave.3.fill")
                        }
                        Button(action: { onRouteAudio("bluetooth") }) {
                            Label("Bluetooth Device", systemImage: "headphones")
                        }
                    } label: {
                        Image(systemName: "speaker.wave.2.fill")
                            .font(.system(size: 14, weight: .semibold))
                            .foregroundStyle(Color.primary)
                            .frame(width: 44, height: 44)
                            .background(Circle().fill(colorScheme == .dark ? Color.white.opacity(0.15) : Color.black.opacity(0.1)))
                    }
                    .menuStyle(.borderlessButton)
                    .frame(width: 44, height: 44)
                    .buttonStyle(CallButtonStyle())

                    Button(action: {
                        NSLog("ClipRelay DEBUG: Ongoing decline button clicked inside CallBannerView!")
                        onDecline()
                    }) {
                        ZStack {
                            Circle().fill(declineRed)
                            Image(systemName: "phone.down.fill")
                                .font(.system(size: 14, weight: .bold))
                                .foregroundStyle(.white)
                        }
                        .frame(width: 44, height: 44)
                        .contentShape(Circle())
                    }
                    .buttonStyle(CallButtonStyle())
                    .shadow(color: declineRed.opacity(0.4), radius: 8, y: 2)
                }
            }
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 16)
        .background {
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .fill(.clear)
                .background(CRHUDMaterial().clipShape(RoundedRectangle(cornerRadius: 20, style: .continuous)))
                .background {
                    RoundedRectangle(cornerRadius: 20, style: .continuous)
                        .fill(colorScheme == .dark ? Color.black.opacity(0.4) : Color.white.opacity(0.4))
                }
                .overlay {
                    RoundedRectangle(cornerRadius: 20, style: .continuous)
                        .strokeBorder(colorScheme == .dark ? Color.white.opacity(0.15) : Color.black.opacity(0.1), lineWidth: 0.5)
                }
                .shadow(color: .black.opacity(0.25), radius: 20, y: 8)
        }
        .onTapGesture {
            NSLog("ClipRelay DEBUG: Entire CallBannerView card tapped!")
        }
    }
}
