import SwiftUI

enum PBTheme {
    static let backgroundTop = Color(red: 0.94, green: 0.93, blue: 0.89)
    static let backgroundBottom = Color(red: 0.83, green: 0.89, blue: 0.88)
    static let surface = Color(red: 0.99, green: 0.98, blue: 0.96).opacity(0.88)
    static let surfaceStrong = Color(red: 0.99, green: 0.98, blue: 0.96)
    static let surfaceSoft = Color(red: 0.96, green: 0.96, blue: 0.93)
    static let ink = Color(red: 0.11, green: 0.13, blue: 0.15)
    static let inkSoft = Color(red: 0.35, green: 0.39, blue: 0.40)
    static let stroke = Color.black.opacity(0.09)
    static let sidebarTop = Color(red: 0.10, green: 0.13, blue: 0.16)
    static let sidebarBottom = Color(red: 0.06, green: 0.08, blue: 0.10)
    static let sidebarCard = Color(red: 0.71, green: 0.94, blue: 0.90).opacity(0.08)
    static let accentBlue = Color(red: 0.06, green: 0.66, blue: 0.74)
    static let accentGreen = Color(red: 0.56, green: 0.78, blue: 0.18)
    static let accentOrange = Color(red: 0.83, green: 0.42, blue: 0.18)
    static let accentPurple = Color(red: 0.78, green: 0.28, blue: 0.23)
    static let accentGold = Color(red: 0.82, green: 0.67, blue: 0.28)
}

struct PBPrimaryButtonStyle: ButtonStyle {
    let tint: Color

    init(tint: Color = PBTheme.accentBlue) {
        self.tint = tint
    }

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 13, weight: .semibold))
            .foregroundStyle(.white)
            .padding(.horizontal, 15)
            .padding(.vertical, 10)
            .background(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .fill(
                        LinearGradient(
                            colors: [tint.opacity(configuration.isPressed ? 0.90 : 1), tint.opacity(0.88)],
                            startPoint: .topLeading,
                            endPoint: .bottomTrailing
                        )
                    )
            )
            .scaleEffect(configuration.isPressed ? 0.985 : 1)
            .shadow(color: tint.opacity(0.14), radius: configuration.isPressed ? 4 : 8, x: 0, y: 4)
            .animation(.easeOut(duration: 0.14), value: configuration.isPressed)
    }
}

struct PBSecondaryButtonStyle: ButtonStyle {
    let dark: Bool

    init(dark: Bool = false) {
        self.dark = dark
    }

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 13, weight: .semibold))
            .foregroundStyle(dark ? Color.white : PBTheme.ink)
            .padding(.horizontal, 15)
            .padding(.vertical, 10)
            .background(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .fill(dark ? Color.white.opacity(configuration.isPressed ? 0.12 : 0.16) : Color.white.opacity(configuration.isPressed ? 0.7 : 0.88))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .stroke(dark ? Color.white.opacity(0.10) : PBTheme.stroke, lineWidth: 1)
            )
            .scaleEffect(configuration.isPressed ? 0.985 : 1)
            .animation(.easeOut(duration: 0.14), value: configuration.isPressed)
    }
}

struct PBPanel<Content: View>: View {
    let dark: Bool
    let content: Content

    init(dark: Bool = false, @ViewBuilder content: () -> Content) {
        self.dark = dark
        self.content = content()
    }

    var body: some View {
        content
            .background(
                RoundedRectangle(cornerRadius: 28, style: .continuous)
                    .fill(
                        dark
                            ? AnyShapeStyle(
                                LinearGradient(
                                    colors: [PBTheme.sidebarTop, PBTheme.sidebarBottom],
                                    startPoint: .topLeading,
                                    endPoint: .bottomTrailing
                                )
                            )
                            : AnyShapeStyle(PBTheme.surface)
                    )
                    .overlay(
                        RoundedRectangle(cornerRadius: 28, style: .continuous)
                            .stroke(dark ? Color.white.opacity(0.06) : PBTheme.stroke, lineWidth: 1)
                    )
                    .overlay(
                        RoundedRectangle(cornerRadius: 28, style: .continuous)
                            .stroke(
                                LinearGradient(
                                    colors: [
                                        (dark ? Color.white : PBTheme.accentBlue).opacity(0.12),
                                        Color.clear,
                                        (dark ? PBTheme.accentGreen : PBTheme.accentGold).opacity(0.08),
                                    ],
                                    startPoint: .topLeading,
                                    endPoint: .bottomTrailing
                                ),
                                lineWidth: 1
                            )
                    )
                    .shadow(color: .black.opacity(dark ? 0.18 : 0.08), radius: dark ? 24 : 30, x: 0, y: 16)
            )
    }
}

struct PBInputChrome: ViewModifier {
    let dark: Bool

    func body(content: Content) -> some View {
        content
            .font(.system(size: 14, weight: .medium))
            .foregroundStyle(dark ? .white : PBTheme.ink)
            .padding(.horizontal, 14)
            .padding(.vertical, 11)
            .background(
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .fill(dark ? Color.white.opacity(0.08) : Color.white.opacity(0.95))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .stroke(dark ? Color.white.opacity(0.10) : PBTheme.stroke, lineWidth: 1)
            )
    }
}

extension View {
    func pbInput(dark: Bool = false) -> some View {
        modifier(PBInputChrome(dark: dark))
    }
}

struct PBBadge: View {
    let text: String
    let tint: Color
    let dark: Bool

    init(_ text: String, tint: Color, dark: Bool = false) {
        self.text = text
        self.tint = tint
        self.dark = dark
    }

    var body: some View {
        Text(text)
            .font(.system(size: 10, weight: .bold))
            .tracking(0.4)
            .foregroundStyle(dark ? Color.white.opacity(0.94) : tint)
            .padding(.horizontal, 8)
            .padding(.vertical, 5)
            .background(
                RoundedRectangle(cornerRadius: 9, style: .continuous)
                    .fill(dark ? tint.opacity(0.22) : tint.opacity(0.12))
            )
    }
}
