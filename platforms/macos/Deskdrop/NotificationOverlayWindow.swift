import AppKit
import Combine
import SwiftUI

@MainActor
final class DeskdropToastWindowManager: NSObject {
    private let store: DeskdropStore
    private let panel: ToastOverlayPanel
    private let hostingView: NSHostingView<ToastOverlayPanelView>
    private var cancellables = Set<AnyCancellable>()

    init(store: DeskdropStore) {
        self.store = store
        self.panel = ToastOverlayPanel()
        self.hostingView = NSHostingView(rootView: ToastOverlayPanelView(store: store))
        super.init()

        hostingView.translatesAutoresizingMaskIntoConstraints = false
        panel.contentView = hostingView

        NotificationCenter.default.addObserver(
            self,
            selector: #selector(layoutPanel),
            name: NSApplication.didChangeScreenParametersNotification,
            object: nil
        )

        store.$toasts
            .receive(on: RunLoop.main)
            .sink { [weak self] toasts in
                self?.handleToastUpdate(toasts)
            }
            .store(in: &cancellables)
    }

    deinit {
        NotificationCenter.default.removeObserver(self)
    }

    private func handleToastUpdate(_ toasts: [ToastItem]) {
        layoutPanel()
        if toasts.isEmpty {
            panel.orderOut(nil)
        } else {
            panel.orderFrontRegardless()
        }
    }

    @objc private func layoutPanel() {
        guard let screen = activeScreen else { return }
        let visible = screen.visibleFrame
        let width: CGFloat = 320
        let height: CGFloat = min(visible.height - 36, 480)
        let frame = NSRect(
            x: visible.maxX - width - 24,
            y: visible.maxY - height - 24,
            width: width,
            height: height
        )
        panel.setFrame(frame, display: false)
    }

    private var activeScreen: NSScreen? {
        if let key = NSApp.keyWindow?.screen {
            return key
        }
        let mouse = NSEvent.mouseLocation
        return NSScreen.screens.first { NSMouseInRect(mouse, $0.frame, false) } ?? NSScreen.main
    }
}

private final class ToastOverlayPanel: NSPanel {
    init() {
        super.init(
            contentRect: NSRect(x: 0, y: 0, width: 372, height: 520),
            styleMask: [.titled, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        titlebarAppearsTransparent = true
        titleVisibility = .hidden
        standardWindowButton(.closeButton)?.isHidden = true
        standardWindowButton(.miniaturizeButton)?.isHidden = true
        standardWindowButton(.zoomButton)?.isHidden = true
        
        level = .statusBar
        hasShadow = false
        isOpaque = false
        backgroundColor = .clear
        hidesOnDeactivate = false
        ignoresMouseEvents = false
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .ignoresCycle]
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}

private struct ToastOverlayPanelView: View {
    @ObservedObject var store: DeskdropStore

    var body: some View {
        VStack(alignment: .trailing, spacing: 12) {
            ForEach(Array(store.toasts.suffix(3).reversed())) { toast in
                ToastOverlayCard(
                    toast: toast,
                    onDismiss: { store.dismissToast(id: toast.id) }
                )
                .transition(.asymmetric(
                    insertion: .move(edge: .trailing).combined(with: .opacity).combined(with: .scale(scale: 0.8)),
                    removal: .move(edge: .trailing).combined(with: .opacity).combined(with: .scale(scale: 0.95))
                ))
            }
            Spacer(minLength: 0)
        }
        .padding(.top, 4)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topTrailing)
        .padding(10)
        .animation(.spring(response: 0.5, dampingFraction: 0.65, blendDuration: 0.1), value: store.toasts.map(\.id))
    }
}

private struct ToastOverlayCard: View {
    let toast: ToastItem
    let onDismiss: () -> Void

    @Environment(\.colorScheme) var colorScheme
    @State private var hovered = false

    var body: some View {
        HStack(alignment: .center, spacing: 14) {
            // Left Icon Column
            ZStack {
                // Clean glass well for icon
                Circle()
                    .fill(Color.black.opacity(0.1))
                    .frame(width: 36, height: 36)
                    .overlay(
                        Circle()
                            .strokeBorder(Color.white.opacity(0.1), lineWidth: 1)
                            .blendMode(.overlay)
                    )
                    .shadow(color: Color.black.opacity(0.2), radius: 2, x: 0, y: 1)
                    
                Image(systemName: toast.systemImage)
                    .font(.system(size: 15, weight: .semibold, design: .rounded))
                    .foregroundStyle(toast.tint)
                    .shadow(color: toast.tint.opacity(0.5), radius: 4, x: 0, y: 2)
            }
            .padding(.vertical, 8)

            // Content Column
            VStack(alignment: .leading, spacing: 3) {
                Text(toast.title)
                    .font(.system(size: 13, weight: .bold, design: .rounded))
                    .foregroundStyle(Color.primary)
                
                Text(toast.body)
                    .font(.system(size: 12, weight: .medium, design: .default))
                    .foregroundStyle(Color.secondary.opacity(0.9))
                    .lineSpacing(2)
                    .fixedSize(horizontal: false, vertical: true)
                
                if let detail = toast.detail, !detail.isEmpty {
                    Text(detail)
                        .font(.system(size: 10, weight: .semibold, design: .monospaced))
                        .foregroundStyle(Color.secondary.opacity(0.6))
                        .padding(.top, 2)
                }

                if let progress = toast.progress {
                    VStack(alignment: .leading, spacing: 6) {
                        CRProgressBar(value: progress, tint: toast.tint, height: 4)
                        Text("\(Int(progress * 100))%")
                            .font(.system(size: 10, weight: .bold, design: .monospaced))
                            .foregroundStyle(toast.tint)
                    }
                    .padding(.top, 6)
                }

                if toast.primaryAction != nil || toast.secondaryAction != nil {
                    HStack(spacing: 8) {
                        if let secondary = toast.secondaryAction {
                            ToastOverlayButton(action: secondary)
                        }
                        Spacer(minLength: 0)
                        if let primary = toast.primaryAction {
                            ToastOverlayButton(action: primary)
                        }
                    }
                    .padding(.top, 8)
                }
            }
            .padding(.vertical, 4)

            Spacer(minLength: 8)

            // Close Button
            Button(action: onDismiss) {
                Image(systemName: "xmark.circle.fill")
                    .font(.system(size: 16, weight: .medium))
                    .foregroundStyle(Color.secondary.opacity(hovered ? 0.8 : 0.4))
            }
            .buttonStyle(.plain)
            .padding(.trailing, 4)
            .opacity(hovered ? 1 : 0.4)
            .animation(.easeOut(duration: 0.15), value: hovered)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .frame(width: 320, alignment: .leading)
        .background {
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .fill(.ultraThinMaterial)
                .overlay(
                    RoundedRectangle(cornerRadius: 16, style: .continuous)
                        .strokeBorder(
                            LinearGradient(
                                colors: [Color.white.opacity(0.4), Color.white.opacity(0.1), Color.white.opacity(0.0)],
                                startPoint: .topLeading,
                                endPoint: .bottomTrailing
                            ),
                            lineWidth: 0.5
                        )
                )
                .shadow(color: Color.black.opacity(0.2), radius: 15, x: 0, y: 8)
        }
        .scaleEffect(hovered ? 1.02 : 1.0)
        .onHover { hovered = $0 }
        .animation(.spring(response: 0.3, dampingFraction: 0.7), value: hovered)
    }
}

private struct ToastOverlayButton: View {
    let action: ToastAction
    @Environment(\.colorScheme) var colorScheme

    var body: some View {
        Button(action: action.handler) {
            Text(action.title.uppercased())
                .font(.system(size: 10, weight: .bold, design: .rounded))
                .tracking(0.5)
                .foregroundStyle(action.role == .secondary ? Color.primary.opacity(0.8) : .white)
                .padding(.horizontal, 16)
                .padding(.vertical, 6)
                .background {
                    Capsule()
                        .fill(
                            action.role == .secondary
                            ? Color.primary.opacity(0.05)
                            : (action.role == .destructive ? Color.red : Color.accentColor)
                        )
                        .overlay {
                            // Inner highlight (Top light)
                            Capsule()
                                .strokeBorder(
                                    LinearGradient(
                                        colors: [Color.white.opacity(0.4), .clear],
                                        startPoint: .top,
                                        endPoint: .bottom
                                    ),
                                    lineWidth: 1
                                )
                            
                            // Secondary button subtle border
                            if action.role == .secondary {
                                Capsule().strokeBorder(Color.primary.opacity(0.15), lineWidth: 1)
                            }
                        }
                        .shadow(color: Color.black.opacity(0.1), radius: 2, y: 1) // Button drop shadow
                }
        }
        .buttonStyle(.plain)
    }
}

