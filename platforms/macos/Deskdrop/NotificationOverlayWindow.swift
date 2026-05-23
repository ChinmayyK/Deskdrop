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
                .transition(.move(edge: .trailing).combined(with: .opacity))
            }
            Spacer(minLength: 0)
        }
        .padding(.top, 4)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topTrailing)
        .padding(10)
        .animation(.spring(response: 0.45, dampingFraction: 0.82, blendDuration: 0.1), value: store.toasts.map(\.id))
    }
}

private struct ToastOverlayCard: View {
    let toast: ToastItem
    let onDismiss: () -> Void

    private let titleColor = Color.primary
    private let bodyColor = Color.secondary

    @Environment(\.colorScheme) var colorScheme
    @State private var hovered = false

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .top, spacing: 12) {
                ZStack {
                    Circle()
                        .fill(toast.tint.opacity(0.12))
                        .frame(width: 32, height: 32)
                    Image(systemName: toast.systemImage)
                        .font(.system(size: 13, weight: .bold))
                        .foregroundStyle(toast.tint)
                        .symbolRenderingMode(.hierarchical)
                }

                VStack(alignment: .leading, spacing: 2) {
                    Text(toast.title)
                        .font(.system(size: 13, weight: .bold))
                        .foregroundStyle(titleColor)
                    Text(toast.body)
                        .font(.system(size: 11.5, weight: .medium))
                        .foregroundStyle(bodyColor)
                        .lineSpacing(1)
                        .fixedSize(horizontal: false, vertical: true)
                    if let detail = toast.detail, !detail.isEmpty {
                        Text(detail)
                            .font(.system(size: 10, weight: .semibold))
                            .foregroundStyle(bodyColor.opacity(0.72))
                    }
                }

                Spacer(minLength: 0)

                Button(action: onDismiss) {
                    ZStack {
                        Circle().fill(Color.white.opacity(0.0)).frame(width: 20, height: 20)
                        Image(systemName: "xmark")
                            .font(.system(size: 10, weight: .bold))
                            .foregroundStyle(bodyColor.opacity(0.8))
                    }
                }
                .buttonStyle(.plain)
                .opacity(hovered ? 1 : 0)
                .animation(.crFast, value: hovered)
            }

            if let progress = toast.progress {
                VStack(alignment: .leading, spacing: 4) {
                    CRProgressBar(value: progress, tint: toast.tint, height: 4)
                    Text("\(Int(progress * 100))% complete")
                        .font(.system(size: 10, weight: .medium))
                        .foregroundStyle(bodyColor.opacity(0.8))
                }
                .padding(.top, 4)
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
                .padding(.top, 4)
            }
        }
        .padding(14)
        .frame(maxWidth: 300, alignment: .leading)
        .background {
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .fill(colorScheme == .dark ? Color(white: 0.1).opacity(0.6) : Color.white.opacity(0.7))
                .background(CRHUDMaterial().clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous)))
                .overlay {
                    RoundedRectangle(cornerRadius: 16, style: .continuous)
                        .strokeBorder(colorScheme == .dark ? Color.white.opacity(0.1) : Color.black.opacity(0.08), lineWidth: 0.5)
                }
                .shadow(color: Color.black.opacity(0.12), radius: 16, y: 8)
        }
        .scaleEffect(hovered ? 1.02 : 1.0)
        .onHover { hovered = $0 }
        .animation(.crSpring, value: hovered)
    }
}

private struct ToastOverlayButton: View {
    let action: ToastAction

    @Environment(\.colorScheme) var colorScheme

    private var fill: Color {
        switch action.role {
        case .primary: return Color.accentColor
        case .secondary: return Color.clear // Handled by background material
        case .positive: return Color(hex: 0x30D158)
        case .destructive: return Color.red
        }
    }

    private var foreground: Color {
        switch action.role {
        case .secondary: return Color.primary
        default: return .white
        }
    }

    var body: some View {
        Button(action.title) { action.handler() }
            .buttonStyle(.plain)
            .font(.system(size: 11.5, weight: .semibold))
            .foregroundStyle(foreground)
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
            .background {
                Capsule()
                    .fill(action.role == .secondary ? Color.clear : fill)
                    .background {
                        if action.role == .secondary {
                            Capsule().fill(.ultraThinMaterial)
                                .overlay {
                                    Capsule().fill(colorScheme == .dark ? Color.white.opacity(0.1) : Color.black.opacity(0.05))
                                }
                        }
                    }
                    .overlay {
                        if action.role == .secondary {
                            Capsule().strokeBorder(colorScheme == .dark ? Color.white.opacity(0.2) : Color.black.opacity(0.15), lineWidth: 0.5)
                        } else {
                            Capsule().strokeBorder(Color.white.opacity(0.2), lineWidth: 0.5)
                        }
                    }
                    .shadow(color: action.role != .secondary ? fill.opacity(0.3) : Color.clear, radius: 4, y: 2)
            }
    }
}
