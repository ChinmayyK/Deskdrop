import SwiftUI

struct RootContainerView: View {
    @ObservedObject var store: DeskdropStore
    @AppStorage("hasCompletedOnboarding") private var hasCompletedOnboarding = false

    var body: some View {
        Group {
            if hasCompletedOnboarding {
                DashboardRootView(store: store)
            } else {
                OnboardingView(onComplete: {
                    hasCompletedOnboarding = true
                })
            }
        }
        .frame(minWidth: 1020, minHeight: 700)
    }
}
struct OnboardingView: View {
    @AppStorage("hasCompletedOnboarding") private var hasCompletedOnboarding = false
    @State private var currentStep = 0
    let onComplete: () -> Void

    var body: some View {
        ZStack {
            CRFluidBackgroundView()
                .ignoresSafeArea()

            VStack(spacing: 0) {
                // Header (Pagination)
                HStack(spacing: 8) {
                    ForEach(0..<3) { step in
                        Circle()
                            .fill(step == currentStep ? CRTheme.brandElectric : CRTheme.strokeSoft)
                            .frame(width: 8, height: 8)
                            .scaleEffect(step == currentStep ? 1.2 : 1.0)
                            .animation(.crSpring, value: currentStep)
                    }
                }
                .padding(.top, 40)
                
                Spacer()

                // Carousel Content
                ZStack {
                    if currentStep == 0 {
                        StepOne()
                            .transition(.asymmetric(insertion: .move(edge: .trailing).combined(with: .opacity), removal: .move(edge: .leading).combined(with: .opacity)))
                    } else if currentStep == 1 {
                        StepTwo()
                            .transition(.asymmetric(insertion: .move(edge: .trailing).combined(with: .opacity), removal: .move(edge: .leading).combined(with: .opacity)))
                    } else if currentStep == 2 {
                        StepThree(onComplete: {
                            hasCompletedOnboarding = true
                        })
                        .transition(.asymmetric(insertion: .move(edge: .trailing).combined(with: .opacity), removal: .move(edge: .leading).combined(with: .opacity)))
                    }
                }
                .animation(.crSpring, value: currentStep)
                .animation(.easeInOut, value: currentStep)
                
                Spacer()
                
                // Footer Navigation
                HStack {
                    if currentStep > 0 {
                        Button("Back") {
                            withAnimation(.crSpring) { currentStep -= 1 }
                        }
                        .buttonStyle(CRSecondaryButtonStyle())
                    } else {
                        Spacer().frame(width: 80) // Placeholder for alignment
                    }
                    
                    Spacer()
                    
                    if currentStep < 2 {
                        Button("Next") {
                            withAnimation(.crSpring) { currentStep += 1 }
                        }
                        .buttonStyle(CRPrimaryButtonStyle(tint: CRTheme.brandElectric))
                    } else {
                        Button("Get Started") {
                            finishOnboarding()
                        }
                        .buttonStyle(CRPrimaryButtonStyle(tint: CRTheme.brandElectric))
                    }
                }
                .padding(.horizontal, 40)
                .padding(.bottom, 40)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
    
    private func finishOnboarding() {
        hasCompletedOnboarding = true
        onComplete()
    }
}

private struct StepOne: View {
    var body: some View {
        VStack(spacing: 24) {
            ZStack {
                Circle().fill(CRTheme.brandElectric.opacity(0.1))
                    .frame(width: 120, height: 120)
                Image(systemName: "paperplane.fill")
                    .font(.system(size: 50))
                    .foregroundStyle(CRTheme.brandElectric)
            }
            
            VStack(spacing: 12) {
                Text("Welcome to Deskdrop")
                    .font(.system(size: 32, weight: .bold, design: .rounded))
                    .foregroundStyle(CRTheme.ink)
                
                Text("Beam files, text, and links instantly across\nmacOS, Windows, Linux, and Android.")
                    .font(.system(size: 16))
                    .foregroundStyle(CRTheme.inkSoft)
                    .multilineTextAlignment(.center)
                    .lineSpacing(4)
            }
        }
    }
}

private struct StepTwo: View {
    var body: some View {
        VStack(spacing: 30) {
            ZStack {
                RoundedRectangle(cornerRadius: 16, style: .continuous)
                    .fill(CRTheme.surfaceElevated)
                    .frame(width: 540, height: 200)
                    .overlay(
                        RoundedRectangle(cornerRadius: 16, style: .continuous)
                            .strokeBorder(CRTheme.stroke, lineWidth: 1)
                    )
                
                LazyVGrid(columns: [GridItem(.flexible(), alignment: .leading), GridItem(.flexible(), alignment: .leading)], spacing: 24) {
                    FeatureRow(icon: "doc.on.clipboard", text: "Universal Clipboard")
                    FeatureRow(icon: "menubar.rectangle", text: "Menu Bar Drag & Drop")
                    FeatureRow(icon: "command", text: "Cmd+K Command Palette")
                    FeatureRow(icon: "clock.arrow.circlepath", text: "Cmd+Shift+V History")
                    FeatureRow(icon: "lock.shield", text: "E2E Encrypted (Noise)")
                    FeatureRow(icon: "eye.slash.fill", text: "Ignores Passwords & OTPs")
                }
                .padding(.horizontal, 40)
            }
            
            VStack(spacing: 12) {
                Text("Packed with Power")
                    .font(.system(size: 28, weight: .bold, design: .rounded))
                    .foregroundStyle(CRTheme.ink)
                
                Text("Deskdrop lives in your menu bar.\nNo accounts, no cloud servers, no limits.")
                    .font(.system(size: 16))
                    .foregroundStyle(CRTheme.inkSoft)
                    .multilineTextAlignment(.center)
                    .lineSpacing(4)
            }
        }
    }
}

private struct FeatureRow: View {
    let icon: String
    let text: String
    var body: some View {
        HStack(spacing: 16) {
            Image(systemName: icon)
                .font(.system(size: 20))
                .foregroundStyle(CRTheme.inkSoft)
                .frame(width: 24)
            Text(text)
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(CRTheme.ink)
        }
    }
}

private struct StepThree: View {
    let onComplete: () -> Void
    
    var body: some View {
        VStack(spacing: 24) {
            ZStack {
                Circle().fill(Color.orange.opacity(0.1))
                    .frame(width: 120, height: 120)
                Image(systemName: "wifi")
                    .font(.system(size: 50))
                    .foregroundStyle(Color.orange)
            }
            
            VStack(spacing: 12) {
                Text("100% Offline & Secure")
                    .font(.system(size: 28, weight: .bold, design: .rounded))
                    .foregroundStyle(CRTheme.ink)
                
                Text("Deskdrop needs Local Network permission to discover\ndevices on your WiFi or Mobile Hotspot. It works\nentirely offline, without an active internet connection.")
                    .font(.system(size: 15))
                    .foregroundStyle(CRTheme.inkSoft)
                    .multilineTextAlignment(.center)
                    .lineSpacing(4)
            }
        }
    }
}
