import AppKit
import Combine
import SwiftUI

@MainActor
final class ClipRelayToastWindowManager: NSObject {
    private let store: ClipRelayStore
    private let panel: ToastOverlayPanel
    private let hostingView: NSHostingView<ToastOverlayPanelView>
    private var cancellables = Set<AnyCancellable>()

    init(store: ClipRelayStore) {
        self.store = store
        self.panel = ToastOverlayPanel()
        self.hostingView = NSHostingView(rootView: ToastOverlayPanelView(store: store))
        super.init()

        hostingView.translatesAutoresizingMaskIntoConstraints = false
        panel.contentView = NSView(frame: .zero)
        panel.contentView?.addSubview(hostingView)
        NSLayoutConstraint.activate([
            hostingView.leadingAnchor.constraint(equalTo: panel.contentView!.leadingAnchor),
            hostingView.trailingAnchor.constraint(equalTo: panel.contentView!.trailingAnchor),
            hostingView.topAnchor.constraint(equalTo: panel.contentView!.topAnchor),
            hostingView.bottomAnchor.constraint(equalTo: panel.contentView!.bottomAnchor),
        ])

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
        let width: CGFloat = 372
        let height: CGFloat = min(visible.height - 36, 520)
        let frame = NSRect(
            x: visible.maxX - width - 18,
            y: visible.maxY - height - 18,
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
            styleMask: [.borderless, .nonactivatingPanel, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
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
    @ObservedObject var store: ClipRelayStore

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

    private let cardBackground = Color(hex: 0xF7F1E8, opacity: 0.98)
    private let cardBorder = Color(hex: 0xDDD3C7, opacity: 0.96)
    private let titleColor = Color(hex: 0x1C1916)
    private let bodyColor = Color(hex: 0x5B554D)

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .top, spacing: 12) {
                ZStack {
                    RoundedRectangle(cornerRadius: 12, style: .continuous)
                        .fill(toast.tint.opacity(0.14))
                        .frame(width: 42, height: 42)
                    Image(systemName: toast.systemImage)
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundStyle(toast.tint)
                        .symbolRenderingMode(.hierarchical)
                }

                VStack(alignment: .leading, spacing: 4) {
                    Text(toast.title)
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundStyle(titleColor)
                    Text(toast.body)
                        .font(.system(size: 12.5, weight: .regular))
                        .foregroundStyle(bodyColor)
                        .lineSpacing(2)
                        .fixedSize(horizontal: false, vertical: true)
                    if let detail = toast.detail, !detail.isEmpty {
                        Text(detail)
                            .font(.system(size: 11.5, weight: .medium))
                            .foregroundStyle(bodyColor.opacity(0.72))
                    }
                }

                Spacer(minLength: 0)

                Button(action: onDismiss) {
                    Image(systemName: "xmark")
                        .font(.system(size: 10.5, weight: .bold))
                        .foregroundStyle(bodyColor.opacity(0.68))
                        .frame(width: 24, height: 24)
                }
                .buttonStyle(.plain)
            }

            if let progress = toast.progress {
                VStack(alignment: .leading, spacing: 6) {
                    CRProgressBar(value: progress, tint: toast.tint, height: 4)
                    Text("\(Int(progress * 100))% complete")
                        .font(.system(size: 11))
                        .foregroundStyle(bodyColor.opacity(0.78))
                }
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
            }
        }
        .padding(14)
        .frame(maxWidth: 344, alignment: .leading)
        .background {
            RoundedRectangle(cornerRadius: 18, style: .continuous)
                .fill(cardBackground)
                .overlay {
                    RoundedRectangle(cornerRadius: 18, style: .continuous)
                        .strokeBorder(cardBorder, lineWidth: 1)
                }
        }
    }
}

private struct ToastOverlayButton: View {
    let action: ToastAction

    private var fill: Color {
        switch action.role {
        case .primary: return Color(hex: 0x2B2A28)
        case .secondary: return Color(hex: 0xEEE5D8)
        case .positive: return Color(hex: 0x355F4F)
        case .destructive: return Color(hex: 0x7D433F)
        }
    }

    private var foreground: Color {
        switch action.role {
        case .secondary: return Color(hex: 0x2B2A28)
        default: return .white
        }
    }

    var body: some View {
        Button(action.title) { action.handler() }
            .buttonStyle(.plain)
            .font(.system(size: 11.5, weight: .semibold))
            .foregroundStyle(foreground)
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background {
                Capsule()
                    .fill(fill)
                    .overlay {
                        if action.role == .secondary {
                            Capsule().strokeBorder(Color(hex: 0xD5CCBE), lineWidth: 1)
                        }
                    }
            }
    }
}
