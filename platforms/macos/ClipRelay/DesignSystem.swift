// DesignSystem.swift — ClipRelay macOS v4
// Canonical design tokens, components, and view modifiers.
// Updated: full adaptive light/dark sidebar + refined component polish.

import SwiftUI
import AppKit

// MARK: - Adaptive Color

extension Color {
    init(light: Color, dark: Color) {
        self.init(nsColor: NSColor(name: nil) { appearance in
            appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
                ? NSColor(dark) : NSColor(light)
        })
    }
    init(hex: UInt32, opacity: Double = 1) {
        self.init(
            red:   Double((hex >> 16) & 0xFF) / 255,
            green: Double((hex >> 8)  & 0xFF) / 255,
            blue:  Double(hex         & 0xFF) / 255,
            opacity: opacity)
    }
}

// MARK: - CRTheme

enum CRTheme {

    // ── Brand ─────────────────────────────────────────────────────────────────
    static let brandElectric = Color(hex: 0x315F8C)
    static let brandViolet   = Color(hex: 0x6B728A)
    static let brandCyan     = Color(hex: 0x9CB3CC)

    // ── Sidebar — fully adaptive ───────────────────────────────────────────────
    static var sidebarBase: Color {
        Color(light: Color(hex: 0xECE6DD), dark: Color(hex: 0x11161B))
    }
    static var sidebarMid: Color {
        Color(light: Color(hex: 0xE1DACE), dark: Color(hex: 0x171E25))
    }
    static var sidebarTop: Color {
        Color(light: Color(hex: 0xF4F0E8), dark: Color(hex: 0x1D2730))
    }

    /// Primary foreground for text/icons drawn on the sidebar surface
    static var sidebarInk: Color {
        Color(light: Color(hex: 0x1B1A17), dark: Color(white: 1, opacity: 0.96))
    }
    /// Secondary / de-emphasised sidebar text
    static var sidebarInkSoft: Color {
        Color(light: Color(hex: 0x6C655C), dark: Color(white: 1, opacity: 0.52))
    }
    /// Very muted sidebar text (timestamps, hints)
    static var sidebarInkSubtle: Color {
        Color(light: Color(hex: 0x978D80), dark: Color(white: 1, opacity: 0.30))
    }
    /// Subtle separator for use inside the sidebar
    static var sidebarDivider: Color {
        Color(light: Color(hex: 0xCEC4B7).opacity(0.9), dark: Color(white: 1, opacity: 0.08))
    }
    /// Selected nav-button pill fill
    static var sidebarSelectedFill: Color {
        Color(light: Color(hex: 0xF7F4ED).opacity(0.96), dark: Color(hex: 0x22303A).opacity(0.92))
    }
    /// Hovered nav-button pill fill
    static var sidebarHoverFill: Color {
        Color(light: Color(hex: 0x000000).opacity(0.035), dark: Color(white: 1, opacity: 0.05))
    }
    /// Selected nav-button border
    static var sidebarSelectedStroke: Color {
        Color(light: Color(hex: 0xC8BFB3).opacity(0.96), dark: Color(hex: 0x3E5561).opacity(0.96))
    }
    static var sidebarSelectedInk: Color {
        Color(light: Color(hex: 0x181613), dark: Color(white: 1, opacity: 0.97))
    }
    static var sidebarSelectedAccent: Color {
        Color(light: Color(hex: 0x244B72), dark: Color(hex: 0xC0D3E6))
    }
    /// Stat-pill fill inside footer
    static var sidebarPillFill: Color {
        Color(light: Color(hex: 0x000000).opacity(0.04), dark: Color(white: 1, opacity: 0.05))
    }
    static var sidebarPillStroke: Color {
        Color(light: Color(hex: 0x000000).opacity(0.06), dark: Color(white: 1, opacity: 0.08))
    }

    // ── Canvas ────────────────────────────────────────────────────────────────
    static var canvasTop:    Color { Color(light: Color(hex: 0xF4F0E8), dark: Color(hex: 0x12171C)) }
    static var canvasBottom: Color { Color(light: Color(hex: 0xE9E2D6), dark: Color(hex: 0x0E1318)) }

    // ── Semantic surfaces ─────────────────────────────────────────────────────
    static var surface:         Color { Color(nsColor: .controlBackgroundColor) }
    static var surfaceStrong:   Color { Color(nsColor: .textBackgroundColor) }
    static var surfaceElevated: Color { Color(nsColor: .windowBackgroundColor) }

    // ── Row states ────────────────────────────────────────────────────────────
    static var rowHover:    Color { Color(light: .black.opacity(0.040), dark: .white.opacity(0.050)) }
    static var rowSelected: Color { Color(light: .black.opacity(0.072), dark: .white.opacity(0.082)) }

    // ── System accent palette ─────────────────────────────────────────────────
    static let accentBlue   = Color(hex: 0x315F8C)
    static let accentGreen  = Color(hex: 0x2D7B58)
    static let accentYellow = Color(hex: 0xD3AB49)
    static let accentOrange = Color(hex: 0xB77833)
    static let accentRed    = Color(hex: 0xB65348)
    static let accentPurple = Color(hex: 0x7B667B)
    static let accentIndigo = Color(hex: 0x5B6D89)
    static let accentTeal   = Color(hex: 0x2F7C7A)
    static let accentMint   = Color(hex: 0x3B8C72)
    static let accentPink   = Color(hex: 0xB96874)
    static let accentGold   = Color(hex: 0xC39A3F)

    // ── Text ──────────────────────────────────────────────────────────────────
    static var ink:       Color { Color(nsColor: .labelColor) }
    static var inkSoft:   Color { Color(nsColor: .secondaryLabelColor) }
    static var inkSubtle: Color { Color(nsColor: .tertiaryLabelColor) }
    static var inkFaint:  Color { Color(nsColor: .quaternaryLabelColor) }

    // ── Borders ───────────────────────────────────────────────────────────────
    static var stroke:     Color { Color(nsColor: .separatorColor) }
    static var strokeSoft: Color { Color(nsColor: .separatorColor).opacity(0.35) }

    // ── Gradients ─────────────────────────────────────────────────────────────
    static var brandGradient: LinearGradient {
        LinearGradient(colors: [brandElectric, brandCyan],
                       startPoint: .topLeading, endPoint: .bottomTrailing)
    }
    static var canvasGradient: LinearGradient {
        LinearGradient(colors: [canvasTop, canvasBottom],
                       startPoint: .topLeading, endPoint: .bottomTrailing)
    }
    /// Adaptive sidebar overlay — subtle in light mode, deep in dark mode
    static var sidebarOverlay: LinearGradient {
        LinearGradient(
            stops: [
                .init(color: sidebarTop.opacity(0.82),  location: 0.0),
                .init(color: sidebarMid.opacity(0.87),  location: 0.5),
                .init(color: sidebarBase.opacity(0.92), location: 1.0)
            ],
            startPoint: .topLeading, endPoint: .bottomTrailing
        )
    }

    // Legacy aliases
    static var backgroundGradient: LinearGradient { canvasGradient }
    static var backgroundTop:      Color           { canvasTop }
    static var backgroundBottom:   Color           { canvasBottom }
    static var sidebarTop_light:   Color           { sidebarTop }
    static var sidebarBottom:      Color           { sidebarBase }
    static var sidebarGradient:    LinearGradient  { sidebarOverlay }
}

typealias PBTheme = CRTheme

// MARK: - Density Mode

enum CRDensityMode {
    case compact, comfortable
    var rowPadding:  CGFloat { self == .compact ? 8  : 12 }
    var cardSpacing: CGFloat { self == .compact ? 4  : 7  }
    var cardRadius:  CGFloat { self == .compact ? 9  : 11 }
}

// MARK: - Animation

extension Animation {
    static let crSpring = Animation.spring(response: 0.24, dampingFraction: 0.86)
    static let crFast   = Animation.spring(response: 0.15, dampingFraction: 0.90)
    static let crSlow   = Animation.spring(response: 0.38, dampingFraction: 0.82)
    static let crBounce = Animation.spring(response: 0.30, dampingFraction: 0.62)
}

// MARK: - NSVisualEffectView Wrappers

struct CRSidebarMaterial: NSViewRepresentable {
    func makeNSView(context: Context) -> NSVisualEffectView {
        let v = NSVisualEffectView()
        v.material = .sidebar; v.blendingMode = .behindWindow; v.state = .active
        return v
    }
    func updateNSView(_ v: NSVisualEffectView, context: Context) {}
}

struct CRHUDMaterial: NSViewRepresentable {
    func makeNSView(context: Context) -> NSVisualEffectView {
        let v = NSVisualEffectView()
        v.material = .popover; v.blendingMode = .behindWindow; v.state = .active
        return v
    }
    func updateNSView(_ v: NSVisualEffectView, context: Context) {}
}

struct CRVisualEffect: NSViewRepresentable {
    var material: NSVisualEffectView.Material = .sidebar
    var blendingMode: NSVisualEffectView.BlendingMode = .behindWindow
    func makeNSView(context: Context) -> NSVisualEffectView {
        let v = NSVisualEffectView()
        v.material = material; v.blendingMode = blendingMode; v.state = .active; return v
    }
    func updateNSView(_ v: NSVisualEffectView, context: Context) {
        v.material = material; v.blendingMode = blendingMode
    }
}

// MARK: - Card Modifier

private struct CRCardModifier: ViewModifier {
    var cornerRadius: CGFloat = 12
    var highlighted:  Bool    = false
    var accentColor:  Color   = CRTheme.accentBlue
    func body(content: Content) -> some View {
        content
            .background {
                RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                    .fill(CRTheme.surfaceStrong)
                    .overlay {
                        RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                            .strokeBorder(
                                highlighted ? accentColor.opacity(0.36) : CRTheme.stroke.opacity(0.40),
                                lineWidth: 0.5)
                    }
                    .shadow(color: .black.opacity(highlighted ? 0.10 : 0.04),
                            radius: highlighted ? 14 : 5, x: 0, y: highlighted ? 4 : 1)
            }
    }
}

extension View {
    func crCard(cornerRadius: CGFloat = 12, highlighted: Bool = false,
                accent: Color = CRTheme.accentBlue) -> some View {
        modifier(CRCardModifier(cornerRadius: cornerRadius, highlighted: highlighted, accentColor: accent))
    }
    func pbCard(cornerRadius: CGFloat = 12, highlighted: Bool = false) -> some View {
        modifier(CRCardModifier(cornerRadius: cornerRadius, highlighted: highlighted))
    }
}

// MARK: - Input Modifier

private struct CRInputModifier: ViewModifier {
    var dark: Bool = false
    var invalid: Bool = false
    func body(content: Content) -> some View {
        content
            .textFieldStyle(.plain)
            .padding(.horizontal, 10).padding(.vertical, 7)
            .background {
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .fill(invalid
                          ? CRTheme.accentRed.opacity(0.06)
                          : (dark ? Color(white: 1, opacity: 0.07) : CRTheme.surface))
                    .overlay {
                        RoundedRectangle(cornerRadius: 8, style: .continuous)
                            .strokeBorder(
                                invalid ? CRTheme.accentRed.opacity(0.55) :
                                (dark ? Color(white: 1, opacity: 0.12) : CRTheme.stroke.opacity(0.65)),
                                lineWidth: invalid ? 1.0 : 0.5)
                    }
            }
            .foregroundStyle(dark ? .white : CRTheme.ink)
    }
}

extension View {
    func crInput(dark: Bool = false, invalid: Bool = false) -> some View {
        modifier(CRInputModifier(dark: dark, invalid: invalid))
    }
    func pbInput(dark: Bool = false) -> some View { modifier(CRInputModifier(dark: dark)) }
}

// MARK: - Search Field

struct CRSearchField: View {
    var placeholder: String
    @Binding var text: String
    var onClear: (() -> Void)? = nil

    var body: some View {
        HStack(spacing: 7) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(CRTheme.inkSoft)
                .frame(width: 14)

            TextField(placeholder, text: $text)
                .textFieldStyle(.plain)
                .font(.system(size: 13))
                .foregroundStyle(CRTheme.ink)

            if !text.isEmpty {
                Button {
                    withAnimation(.crFast) { text = "" }
                    onClear?()
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .font(.system(size: 13))
                        .foregroundStyle(CRTheme.inkSoft)
                }
                .buttonStyle(.plain)
                .transition(.scale(scale: 0.75).combined(with: .opacity))
            }
        }
        .padding(.horizontal, 9).padding(.vertical, 6.5)
        .background {
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(CRTheme.surface)
                .overlay {
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .strokeBorder(CRTheme.stroke.opacity(0.55), lineWidth: 0.5)
                }
        }
        .animation(.crFast, value: text.isEmpty)
    }
}

// MARK: - Glow

struct GlowModifier: ViewModifier {
    var color: Color; var radius: CGFloat
    func body(content: Content) -> some View {
        content
            .shadow(color: color.opacity(0.48), radius: radius / 2)
            .shadow(color: color.opacity(0.20), radius: radius)
    }
}
extension View {
    func crGlow(_ color: Color, radius: CGFloat = 8) -> some View {
        modifier(GlowModifier(color: color, radius: radius))
    }
}

// MARK: - Button Styles

struct CRPrimaryButtonStyle: ButtonStyle {
    var tint: Color = CRTheme.accentBlue
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 12.5, weight: .semibold))
            .foregroundStyle(.white)
            .padding(.horizontal, 13).padding(.vertical, 6)
            .background {
                RoundedRectangle(cornerRadius: 7, style: .continuous)
                    .fill(tint).brightness(configuration.isPressed ? -0.06 : 0)
            }
            .scaleEffect(configuration.isPressed ? 0.97 : 1.0)
            .animation(.crFast, value: configuration.isPressed)
    }
}

struct CRSecondaryButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 12.5, weight: .medium))
            .foregroundStyle(CRTheme.ink)
            .padding(.horizontal, 13).padding(.vertical, 6)
            .background {
                RoundedRectangle(cornerRadius: 7, style: .continuous)
                    .fill(CRTheme.surface)
                    .overlay {
                        RoundedRectangle(cornerRadius: 7, style: .continuous)
                            .strokeBorder(CRTheme.stroke.opacity(0.65), lineWidth: 0.5)
                    }
                    .opacity(configuration.isPressed ? 0.75 : 1.0)
            }
            .scaleEffect(configuration.isPressed ? 0.97 : 1.0)
            .animation(.crFast, value: configuration.isPressed)
    }
}

struct CRDestructiveButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 12.5, weight: .semibold))
            .foregroundStyle(CRTheme.accentRed)
            .padding(.horizontal, 13).padding(.vertical, 6)
            .background {
                RoundedRectangle(cornerRadius: 7, style: .continuous)
                    .fill(CRTheme.accentRed.opacity(0.08))
                    .overlay {
                        RoundedRectangle(cornerRadius: 7, style: .continuous)
                            .strokeBorder(CRTheme.accentRed.opacity(0.18), lineWidth: 0.5)
                    }
                    .opacity(configuration.isPressed ? 0.75 : 1.0)
            }
            .scaleEffect(configuration.isPressed ? 0.97 : 1.0)
            .animation(.crFast, value: configuration.isPressed)
    }
}

typealias PBPrimaryButtonStyle     = CRPrimaryButtonStyle
typealias PBSecondaryButtonStyle   = CRSecondaryButtonStyle
typealias PBDestructiveButtonStyle = CRDestructiveButtonStyle

// MARK: - Tag / Badge

struct CRTag: View {
    let text: String
    let tint: Color
    var filled: Bool = false
    var body: some View {
        Text(text)
            .font(.system(size: 10, weight: .semibold))
            .tracking(0.10)
            .foregroundStyle(filled ? .white : tint.opacity(0.88))
            .padding(.horizontal, 6.5).padding(.vertical, 2.5)
            .background { Capsule().fill(filled ? tint : tint.opacity(0.10)) }
    }
}

/// Adaptive sidebar badge — uses sidebar semantic tokens so it reads in both modes
struct CRNumericBadge: View {
    let count: Int
    var body: some View {
        if count > 0 {
            Text("\(min(count, 99))")
                .font(.system(size: 10, weight: .bold, design: .rounded))
                .foregroundStyle(CRTheme.sidebarInkSoft)
                .padding(.horizontal, 5.5).padding(.vertical, 2)
                .background {
                    Capsule().fill(CRTheme.sidebarPillFill)
                        .overlay { Capsule().strokeBorder(CRTheme.sidebarPillStroke, lineWidth: 0.5) }
                }
        }
    }
}

typealias CRBadge = CRTag
typealias PBBadge = CRTag

// MARK: - Shortcut Hint (sidebar ⌘N labels)

struct CRShortcutHint: View {
    let shortcut: String
    /// Pass `false` in the adaptive sidebar so it uses sidebar semantic colours.
    var dark: Bool = true
    var body: some View {
        Text(shortcut)
            .font(.system(size: 10, weight: .medium, design: .rounded))
            .foregroundStyle(dark ? Color(white: 1, opacity: 0.24) : CRTheme.sidebarInkSubtle)
            .padding(.horizontal, 5).padding(.vertical, 2)
            .background {
                RoundedRectangle(cornerRadius: 4, style: .continuous)
                    .fill(dark ? Color(white: 1, opacity: 0.06) : CRTheme.sidebarPillFill)
                    .overlay {
                        RoundedRectangle(cornerRadius: 4, style: .continuous)
                            .strokeBorder(
                                dark ? Color(white: 1, opacity: 0.08) : CRTheme.sidebarPillStroke,
                                lineWidth: 0.5)
                    }
            }
    }
}

// MARK: - Status Dot

struct StatusDot: View {
    var isOnline: Bool
    var size: CGFloat = 8
    @State private var pulse = false
    var body: some View {
        ZStack {
            if isOnline {
                Circle()
                    .fill(CRTheme.accentGreen.opacity(0.20))
                    .frame(width: size + 9, height: size + 9)
                    .scaleEffect(pulse ? 2.1 : 1.0)
                    .opacity(pulse ? 0 : 0.45)
                    .animation(.easeOut(duration: 1.6).repeatForever(autoreverses: false), value: pulse)
            }
            Circle().fill(isOnline ? CRTheme.accentGreen : CRTheme.inkSubtle).frame(width: size, height: size)
        }
        .onAppear { if isOnline { pulse = true } }
        .onChange(of: isOnline) { v in pulse = v }
    }
}

// MARK: - Device Avatar

struct DeviceAvatar: View {
    let name: String; let platform: String?
    var size: CGFloat = 36; var color: Color = CRTheme.accentBlue
    private var initials: String {
        let w = name.split(separator: " ")
        return w.count >= 2 ? String(w[0].prefix(1) + w[1].prefix(1)).uppercased()
                            : String(name.prefix(2)).uppercased()
    }
    var body: some View {
        ZStack {
            Circle().fill(color.opacity(0.12))
                .overlay { Circle().strokeBorder(color.opacity(0.16), lineWidth: 0.5) }
                .frame(width: size, height: size)
            Text(initials)
                .font(.system(size: size * 0.33, weight: .semibold, design: .rounded))
                .foregroundStyle(color)
        }
    }
}

// MARK: - App Icon Mark

struct CRAppIconMark: View {
    var size: CGFloat = 34
    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: size * 0.30, style: .continuous)
                .fill(CRTheme.brandGradient)
                .frame(width: size, height: size)
                .shadow(color: CRTheme.brandElectric.opacity(0.42), radius: size * 0.35, y: size * 0.12)
            Image(systemName: "arrow.left.arrow.right.circle.fill")
                .font(.system(size: size * 0.44, weight: .semibold))
                .foregroundStyle(.white.opacity(0.96))
                .symbolRenderingMode(.hierarchical)
        }
    }
}

// MARK: - Icon Chip

struct CRIconChip: View {
    let systemName: String; let tint: Color
    var size: CGFloat = 28; var radius: CGFloat? = nil
    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: radius ?? size * 0.32, style: .continuous)
                .fill(tint.opacity(0.10)).frame(width: size, height: size)
            Image(systemName: systemName)
                .font(.system(size: size * 0.42, weight: .semibold))
                .foregroundStyle(tint).symbolRenderingMode(.hierarchical)
        }
    }
}

// MARK: - Section Header

struct CRSectionHeader: View {
    let eyebrow: String; let title: String; var subtitle: String? = nil
    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 5) {
                Capsule().fill(CRTheme.brandGradient).frame(width: 2.5, height: 11)
                Text(eyebrow.uppercased())
                    .font(.system(size: 10, weight: .bold)).tracking(1.0)
                    .foregroundStyle(CRTheme.brandElectric)
            }
            Text(title)
                .font(.system(size: 24, weight: .bold)).foregroundStyle(CRTheme.ink).tracking(-0.4)
            if let subtitle {
                Text(subtitle).font(.system(size: 13)).foregroundStyle(CRTheme.inkSoft)
                    .fixedSize(horizontal: false, vertical: true).lineSpacing(1.5)
            }
        }
    }
}

// MARK: - Empty State

struct CREmptyState: View {
    let systemImage: String; let title: String; let message: String
    var accent: Color = CRTheme.accentBlue
    var actionLabel: String? = nil
    var onAction: (() -> Void)? = nil
    var body: some View {
        VStack(spacing: 14) {
            ZStack {
                Circle().fill(accent.opacity(0.06)).frame(width: 72, height: 72)
                Circle().strokeBorder(accent.opacity(0.10), lineWidth: 0.5).frame(width: 72, height: 72)
                Image(systemName: systemImage)
                    .font(.system(size: 24, weight: .light)).foregroundStyle(accent.opacity(0.48))
                    .symbolRenderingMode(.hierarchical)
            }
            VStack(spacing: 5) {
                Text(title).font(.system(size: 13.5, weight: .semibold)).foregroundStyle(CRTheme.ink)
                Text(message).font(.system(size: 12)).foregroundStyle(CRTheme.inkSoft)
                    .multilineTextAlignment(.center).frame(maxWidth: 280).lineSpacing(2)
            }
            if let label = actionLabel, let action = onAction {
                Button(label, action: action).buttonStyle(CRPrimaryButtonStyle(tint: accent))
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity).padding(.vertical, 48)
    }
}

// MARK: - Toast Stack

struct CRToastStack: View {
    let toasts: [ToastItem]
    var body: some View {
        VStack(alignment: .trailing, spacing: 6) {
            ForEach(toasts.suffix(3)) { toast in
                HStack(spacing: 9) {
                    RoundedRectangle(cornerRadius: 2).fill(toast.tint).frame(width: 3, height: 30)
                    VStack(alignment: .leading, spacing: 1) {
                        Text(toast.title).font(.system(size: 12.5, weight: .semibold)).foregroundStyle(CRTheme.ink)
                        Text(toast.body).font(.system(size: 11)).foregroundStyle(CRTheme.inkSoft)
                    }
                    Spacer(minLength: 0)
                }
                .padding(.leading, 4).padding(.trailing, 14).padding(.vertical, 8)
                .background {
                    RoundedRectangle(cornerRadius: 11, style: .continuous)
                        .fill(.regularMaterial)
                        .overlay {
                            RoundedRectangle(cornerRadius: 11, style: .continuous)
                                .strokeBorder(CRTheme.stroke.opacity(0.30), lineWidth: 0.5)
                        }
                        .shadow(color: .black.opacity(0.07), radius: 12, y: 3)
                }
                .frame(maxWidth: 270)
                .transition(.asymmetric(insertion: .move(edge: .trailing).combined(with: .opacity),
                                        removal:   .move(edge: .trailing).combined(with: .opacity)))
            }
        }
        .animation(.crSpring, value: toasts.count)
    }
}

// MARK: - Sidebar Nav Button

struct SidebarNavButton: View {
    let icon: String; let label: String
    var badge: Int = 0; var shortcut: String = ""
    var isSelected: Bool; let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                Image(systemName: isSelected ? (icon + ".fill") : icon)
                    .font(.system(size: 13.5, weight: isSelected ? .semibold : .regular))
                    .foregroundStyle(isSelected ? CRTheme.sidebarSelectedAccent : CRTheme.sidebarInkSoft)
                    .symbolRenderingMode(.hierarchical)
                    .frame(width: 18, alignment: .center)

                Text(label)
                    .font(.system(size: 13, weight: isSelected ? .semibold : .regular))
                    .foregroundStyle(isSelected ? CRTheme.sidebarSelectedInk : CRTheme.sidebarInkSoft)

                Spacer(minLength: 0)

                if hovered && !shortcut.isEmpty {
                    CRShortcutHint(shortcut: shortcut, dark: false)
                        .transition(.opacity.combined(with: .scale(scale: 0.9)))
                } else {
                    CRNumericBadge(count: badge)
                }
            }
            .padding(.horizontal, 10).padding(.vertical, 7)
            .frame(maxWidth: .infinity, minHeight: 30)
            .background {
                RoundedRectangle(cornerRadius: 7, style: .continuous)
                    .fill(isSelected ? CRTheme.sidebarSelectedFill
                                     : (hovered ? CRTheme.sidebarHoverFill : .clear))
                    .overlay {
                        if isSelected {
                            RoundedRectangle(cornerRadius: 7, style: .continuous)
                                .strokeBorder(CRTheme.sidebarSelectedStroke, lineWidth: 0.5)
                        }
                    }
                    .shadow(color: .black.opacity(isSelected ? 0.05 : 0),
                            radius: isSelected ? 3 : 0, x: 0, y: isSelected ? 1 : 0)
            }
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .animation(.crFast, value: isSelected)
        .animation(.crFast, value: hovered)
    }
}

// MARK: - Sidebar Stat Pill

struct SidebarStatPill: View {
    let icon: String; let value: String; let label: String
    var body: some View {
        HStack(spacing: 5) {
            Image(systemName: icon).font(.system(size: 9, weight: .semibold))
                .foregroundStyle(CRTheme.sidebarInkSubtle)
            Text(value).font(.system(size: 12, weight: .bold, design: .rounded))
                .foregroundStyle(CRTheme.sidebarInk.opacity(0.80))
            Text(label).font(.system(size: 10)).foregroundStyle(CRTheme.sidebarInkSubtle)
        }
        .padding(.horizontal, 9).padding(.vertical, 5)
        .background {
            RoundedRectangle(cornerRadius: 7, style: .continuous)
                .fill(CRTheme.sidebarPillFill)
                .overlay {
                    RoundedRectangle(cornerRadius: 7, style: .continuous)
                        .strokeBorder(CRTheme.sidebarPillStroke, lineWidth: 0.5)
                }
        }
    }
}

struct DevicePill: View {
    let text: String; let tint: Color
    var body: some View { CRTag(text: text, tint: tint) }
}

// MARK: - Dividers

struct CRDivider: View {
    var inset: CGFloat = 0
    var body: some View {
        Rectangle().fill(CRTheme.stroke.opacity(0.45)).frame(height: 0.5).padding(.leading, inset)
    }
}

/// Adaptive divider for use inside the sidebar
struct CRDividerDark: View {
    var inset: CGFloat = 0
    var body: some View {
        Rectangle().fill(CRTheme.sidebarDivider).frame(height: 0.5).padding(.leading, inset)
    }
}

// MARK: - Keyboard Chip

struct KbdChip: View {
    let key: String; var dark: Bool = true
    init(_ key: String, dark: Bool = true) { self.key = key; self.dark = dark }
    var body: some View {
        Text(key)
            .font(.system(size: 10.5, weight: .semibold, design: .rounded))
            .foregroundStyle(dark ? Color(white: 1, opacity: 0.28) : CRTheme.inkSubtle)
            .padding(.horizontal, 5.5).padding(.vertical, 2.5)
            .background {
                RoundedRectangle(cornerRadius: 5, style: .continuous)
                    .fill(dark ? Color(white: 1, opacity: 0.07) : CRTheme.surface)
                    .overlay {
                        RoundedRectangle(cornerRadius: 5, style: .continuous)
                            .strokeBorder(dark ? Color(white: 1, opacity: 0.10) : CRTheme.stroke.opacity(0.55),
                                          lineWidth: 0.5)
                    }
            }
    }
}

// MARK: - Inline Progress Bar

struct CRProgressBar: View {
    var value: Double; var tint: Color = CRTheme.accentIndigo; var height: CGFloat = 3
    var body: some View {
        GeometryReader { geo in
            ZStack(alignment: .leading) {
                Capsule().fill(tint.opacity(0.14)).frame(height: height)
                Capsule().fill(tint)
                    .frame(width: geo.size.width * max(0, min(value, 1)), height: height)
                    .animation(.crSpring, value: value)
            }
        }
        .frame(height: height)
    }
}

// MARK: - Day Header (for grouped lists)

struct CRDayHeader: View {
    let date: Date
    private var label: String {
        if Calendar.current.isDateInToday(date)     { return "Today" }
        if Calendar.current.isDateInYesterday(date) { return "Yesterday" }
        let f = DateFormatter()
        f.dateFormat = Calendar.current.component(.year, from: date) == Calendar.current.component(.year, from: Date())
            ? "EEEE, MMMM d" : "MMMM d, yyyy"
        return f.string(from: date)
    }
    var body: some View {
        HStack(spacing: 8) {
            Text(label)
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(CRTheme.inkSubtle)
            Rectangle().fill(CRTheme.stroke.opacity(0.35)).frame(height: 0.5)
        }
        .padding(.vertical, 6)
    }
}

// MARK: - Section Toolbar (sticky top bar for detail panes)

struct CRSectionToolbar<TrailingContent: View>: View {
    let title:    String
    let subtitle: String
    @ViewBuilder var trailing: () -> TrailingContent

    var body: some View {
        VStack(spacing: 0) {
            HStack(alignment: .center) {
                VStack(alignment: .leading, spacing: 1) {
                    Text(title).font(.system(size: 15, weight: .semibold)).foregroundStyle(CRTheme.ink)
                    Text(subtitle).font(.system(size: 11)).foregroundStyle(CRTheme.inkSoft)
                }
                Spacer()
                trailing()
            }
            .padding(.horizontal, 20).padding(.vertical, 11)
            CRDivider()
        }
        .background(CRTheme.surfaceElevated)
    }
}
