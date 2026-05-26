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
        let width: CGFloat = 400
        let height: CGFloat = min(visible.height - 16, 400)
        let frame = NSRect(
            x: visible.minX + (visible.width - width) / 2, // Center horizontally
            y: visible.maxY - height - 12, // Pin to top
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
            contentRect: NSRect(x: 0, y: 0, width: 400, height: 400),
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
        VStack(alignment: .center, spacing: 8) {
            ForEach(Array(store.toasts.suffix(3).reversed())) { toast in
                ToastOverlayCard(
                    toast: toast,
                    onDismiss: { store.dismissToast(id: toast.id) }
                )
                .transition(.asymmetric(
                    insertion: .move(edge: .top).combined(with: .opacity).combined(with: .scale(scale: 0.85)),
                    removal: .opacity.combined(with: .scale(scale: 0.95))
                ))
                .zIndex(Double(toast.id.hashValue))
            }
            Spacer(minLength: 0)
        }
        .padding(.top, 4)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .padding(10)
        .animation(.spring(response: 0.45, dampingFraction: 0.65, blendDuration: 0.1), value: store.toasts.map(\.id))
    }
}

private struct ToastOverlayCard: View {
    let toast: ToastItem
    let onDismiss: () -> Void

    @Environment(\.colorScheme) var colorScheme
    @State private var hovered = false

    var body: some View {
        HStack(alignment: .center, spacing: 12) {
            // Left Icon
            Image(systemName: toast.systemImage)
                .font(.system(size: 16, weight: .semibold, design: .rounded))
                .foregroundStyle(toast.tint)
                .frame(width: 20)

            // Content Column
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text(toast.title)
                        .font(.system(size: 13, weight: .bold, design: .rounded))
                        .foregroundStyle(Color.white)
                        .lineLimit(1)
                    
                    if let detail = toast.detail, !detail.isEmpty {
                        Text("• " + detail)
                            .font(.system(size: 11, weight: .medium, design: .default))
                            .foregroundStyle(Color.white.opacity(0.5))
                            .lineLimit(1)
                    }
                }
                
                if !toast.body.isEmpty {
                    Text(toast.body)
                        .font(.system(size: 12, weight: .medium, design: .default))
                        .foregroundStyle(Color.white.opacity(0.7))
                        .lineLimit(2)
                }

                if let progress = toast.progress {
                    CRProgressBar(value: progress, tint: toast.tint, height: 4)
                        .padding(.top, 4)
                }

                if toast.primaryAction != nil || toast.secondaryAction != nil {
                    HStack(spacing: 8) {
                        if let secondary = toast.secondaryAction {
                            ToastOverlayButton(action: secondary)
                        }
                        if let primary = toast.primaryAction {
                            ToastOverlayButton(action: primary)
                        }
                    }
                    .padding(.top, 6)
                }
            }

            Spacer(minLength: 8)

            // Close Button
            if hovered {
                Button(action: onDismiss) {
                    Image(systemName: "xmark.circle.fill")
                        .font(.system(size: 14, weight: .medium))
                        .foregroundStyle(Color.white.opacity(0.4))
                }
                .buttonStyle(.plain)
                .transition(.opacity)
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .frame(minWidth: 260, maxWidth: 360, alignment: .leading)
        .background {
            Capsule(style: .continuous)
                .fill(Color.black.opacity(0.85))
                .overlay(
                    Capsule(style: .continuous)
                        .strokeBorder(Color.white.opacity(0.1), lineWidth: 0.5)
                )
                .shadow(color: Color.black.opacity(0.4), radius: 20, x: 0, y: 10)
        }
        .scaleEffect(hovered ? 1.01 : 1.0)
        .onHover { isHovering in withAnimation(.easeOut(duration: 0.15)) { hovered = isHovering } }
    }
}

private struct ToastOverlayButton: View {
    let action: ToastAction
    @State private var isHovered = false

    var body: some View {
        Button(action: action.handler) {
            Text(action.title)
                .font(.system(size: 11, weight: .semibold, design: .rounded))
                .foregroundStyle(action.role == .secondary ? Color.white.opacity(0.8) : Color.black)
                .padding(.horizontal, 14)
                .padding(.vertical, 6)
                .background {
                    Capsule()
                        .fill(
                            action.role == .secondary
                            ? Color.white.opacity(isHovered ? 0.2 : 0.1)
                            : toastTintOrAccent()
                        )
                }
        }
        .buttonStyle(.plain)
        .scaleEffect(isHovered ? 1.05 : 1.0)
        .onHover { isHovering in withAnimation(.easeOut(duration: 0.15)) { isHovered = isHovering } }
    }
    
    private func toastTintOrAccent() -> Color {
        if action.role == .destructive { return .red }
        return .white
    }
}

