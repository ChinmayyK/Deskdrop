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
        HStack(alignment: .top, spacing: 16) {
            // Left Icon Column (Glowing Orb Style)
            ZStack {
                Circle()
                    .fill(toast.tint.opacity(0.15))
                    .frame(width: 42, height: 42)
                    .overlay(
                        Circle()
                            .strokeBorder(toast.tint.opacity(0.4), lineWidth: 1)
                            .blendMode(.screen)
                    )
                    .shadow(color: toast.tint.opacity(0.4), radius: 8, x: 0, y: 4)
                    
                Image(systemName: toast.systemImage)
                    .font(.system(size: 18, weight: .semibold, design: .rounded))
                    .foregroundStyle(toast.tint)
            }
            .padding(.top, 4)

            // Content Column
            VStack(alignment: .leading, spacing: 4) {
                HStack(alignment: .top) {
                    Text(toast.title)
                        .font(.system(size: 14, weight: .semibold, design: .rounded))
                        .foregroundStyle(Color.primary)
                        .lineLimit(1)
                    Spacer()
                    // Close Button
                    Button(action: onDismiss) {
                        Image(systemName: "xmark")
                            .font(.system(size: 12, weight: .bold))
                            .foregroundStyle(Color.secondary.opacity(hovered ? 0.8 : 0.0))
                    }
                    .buttonStyle(.plain)
                    .padding(.top, 2)
                    .padding(.trailing, 2)
                }
                
                Text(toast.body)
                    .font(.system(size: 13, weight: .medium, design: .default))
                    .foregroundStyle(Color.secondary.opacity(0.95))
                    .lineSpacing(3)
                    .fixedSize(horizontal: false, vertical: true)
                
                if let detail = toast.detail, !detail.isEmpty {
                    Text(detail)
                        .font(.system(size: 11, weight: .medium, design: .monospaced))
                        .foregroundStyle(toast.tint.opacity(0.8))
                        .padding(.top, 2)
                }

                if let progress = toast.progress {
                    VStack(alignment: .leading, spacing: 6) {
                        CRProgressBar(value: progress, tint: toast.tint, height: 4)
                        Text("\(Int(progress * 100))%")
                            .font(.system(size: 11, weight: .bold, design: .monospaced))
                            .foregroundStyle(toast.tint)
                    }
                    .padding(.top, 8)
                }

                if toast.primaryAction != nil || toast.secondaryAction != nil {
                    HStack(spacing: 12) {
                        if let secondary = toast.secondaryAction {
                            ToastOverlayButton(action: secondary)
                        }
                        Spacer(minLength: 0)
                        if let primary = toast.primaryAction {
                            ToastOverlayButton(action: primary)
                        }
                    }
                    .padding(.top, 12)
                }
            }
            .padding(.vertical, 2)
        }
        .padding(.leading, 16)
        .padding(.trailing, 12)
        .padding(.vertical, 14)
        .frame(width: 340, alignment: .leading)
        .background {
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .fill(.ultraThinMaterial)
                .overlay(
                    RoundedRectangle(cornerRadius: 20, style: .continuous)
                        .strokeBorder(
                            LinearGradient(
                                colors: [Color.white.opacity(0.5), Color.white.opacity(0.1), Color.white.opacity(0.0)],
                                startPoint: .topLeading,
                                endPoint: .bottomTrailing
                            ),
                            lineWidth: 1
                        )
                )
                .overlay(
                    // Subtle tint glow on the edge
                    RoundedRectangle(cornerRadius: 20, style: .continuous)
                        .strokeBorder(toast.tint.opacity(hovered ? 0.3 : 0.1), lineWidth: 1)
                        .blendMode(.screen)
                )
                .shadow(color: Color.black.opacity(0.15), radius: 24, x: 0, y: 12)
                .shadow(color: toast.tint.opacity(0.05), radius: 10, x: 0, y: 0)
        }
        .scaleEffect(hovered ? 1.02 : 1.0)
        .onHover { hovered = $0 }
        .animation(.spring(response: 0.35, dampingFraction: 0.65), value: hovered)
    }
}

private struct ToastOverlayButton: View {
    let action: ToastAction
    @Environment(\.colorScheme) var colorScheme
    @State private var isHovered = false

    var body: some View {
        Button(action: action.handler) {
            Text(action.title)
                .font(.system(size: 12, weight: .semibold, design: .rounded))
                .foregroundStyle(action.role == .secondary ? Color.primary.opacity(0.8) : .white)
                .padding(.horizontal, 18)
                .padding(.vertical, 8)
                .background {
                    Capsule()
                        .fill(
                            action.role == .secondary
                            ? Color.primary.opacity(isHovered ? 0.1 : 0.05)
                            : (action.role == .destructive ? Color.red : Color.accentColor)
                        )
                        .overlay {
                            if action.role != .secondary {
                                Capsule()
                                    .strokeBorder(
                                        LinearGradient(
                                            colors: [Color.white.opacity(0.4), .clear],
                                            startPoint: .top,
                                            endPoint: .bottom
                                        ),
                                        lineWidth: 1
                                    )
                            } else {
                                Capsule().strokeBorder(Color.primary.opacity(0.15), lineWidth: 1)
                            }
                        }
                        .shadow(color: Color.black.opacity(action.role == .secondary ? 0 : 0.15), radius: 4, y: 2)
                }
                .scaleEffect(isHovered ? 1.03 : 1.0)
                .animation(.easeOut(duration: 0.15), value: isHovered)
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
    }
}

