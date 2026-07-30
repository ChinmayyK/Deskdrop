import SwiftUI

struct RootContainerView: View {
    @ObservedObject var store: DeskdropStore
    @AppStorage("hasCompletedOnboarding") private var hasCompletedOnboarding = false

    var body: some View {
        Group {
            if hasCompletedOnboarding {
                CommandCenterRootView(store: store)
            } else {
                OnboardingView(store: store, onComplete: {
                    hasCompletedOnboarding = true
                })
            }
        }
        .frame(minWidth: 1200, minHeight: 760)
        .ignoresSafeArea(.all, edges: .top)
    }
}

struct OnboardingView: View {
    @ObservedObject var store: DeskdropStore
    @State private var selectedPeerId: String? = nil
    @State private var sessionStartTime: Date = Date()
    
    private var selectedPeer: PeerViewModel? {
        store.peers.first { $0.id == selectedPeerId }
    }
    
    private var currentStep: Int {
        guard let peer = selectedPeer else { return 0 }
        if !peer.trusted { return 1 }
        return 1 // We handle the trusted state via onChange now
    }

    let onComplete: () -> Void

    var body: some View {
        ZStack {
            CRFluidBackgroundView().ignoresSafeArea()

            VStack(spacing: 0) {
                Spacer()

                ZStack {
                    if currentStep == 0 {
                        StepOneFindDevice(store: store, selectedPeerId: $selectedPeerId)
                            .transition(.asymmetric(insertion: .move(edge: .trailing).combined(with: .opacity), removal: .move(edge: .leading).combined(with: .opacity)))
                    } else if currentStep == 1 {
                        StepTwoVerify(store: store, selectedPeer: selectedPeer, onCancel: { selectedPeerId = nil })
                            .transition(.asymmetric(insertion: .move(edge: .trailing).combined(with: .opacity), removal: .move(edge: .leading).combined(with: .opacity)))
                    }
                }
                .animation(.crSpring, value: currentStep)
                
                Spacer()
                
                // Footer Navigation (Simplified)
                HStack {
                    if currentStep > 0 {
                        Button("Cancel") {
                            withAnimation(.crSpring) { selectedPeerId = nil }
                        }
                        .buttonStyle(CRSecondaryButtonStyle())
                    }
                    Spacer()
                }
                .padding(.horizontal, 40)
                .padding(.bottom, 40)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .onChange(of: selectedPeer?.trusted) { trusted in
            if trusted == true {
                onComplete()
            }
        }
    }
}

private struct StepOneFindDevice: View {
    @ObservedObject var store: DeskdropStore
    @Binding var selectedPeerId: String?
    @State private var showingQR = false
    
    var body: some View {
        VStack(spacing: 24) {
            Text("Step 1: Find a device")
                .font(.system(size: 28, weight: .bold, design: .rounded))
            Text("Make sure Deskdrop is running on your phone or another computer.")
                .foregroundStyle(CRTheme.inkSoft)
            
            Button("Show QR Code") {
                showingQR = true
            }
            .buttonStyle(CRPrimaryButtonStyle(tint: CRTheme.accentGreen))
            .sheet(isPresented: $showingQR) {
                QRCodePairingSheet(store: store)
            }
            
            ScrollView {
                VStack(spacing: 8) {
                    if store.peers.isEmpty {
                        VStack(spacing: 24) {
                            RadarPulseView()
                            Text("Scanning local network...").foregroundStyle(CRTheme.inkSoft).font(.system(size: 14, weight: .medium))
                        }
                        .padding(.vertical, 32)
                    } else {
                        ForEach(store.peers) { peer in
                            Button {
                                selectedPeerId = peer.id
                                store.connectAndPair(deviceId: peer.id)
                            } label: {
                                HStack {
                                    if #available(macOS 14.0, *) {
                                        Image(systemName: peer.displayName.lowercased().contains("mac") ? "laptopcomputer" : "smartphone")
                                            .symbolEffect(.bounce, value: selectedPeerId == peer.id)
                                    } else {
                                        Image(systemName: peer.displayName.lowercased().contains("mac") ? "laptopcomputer" : "smartphone")
                                    }
                                    Text(peer.displayName).font(.system(size: 16, weight: .semibold))
                                    Spacer()
                                }
                                .padding()
                                .background(selectedPeerId == peer.id ? CRTheme.brandElectric.opacity(0.1) : CRTheme.surfaceElevated)
                                .cornerRadius(12)
                                .overlay(RoundedRectangle(cornerRadius: 12).stroke(selectedPeerId == peer.id ? CRTheme.brandElectric : CRTheme.stroke, lineWidth: 1))
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }
                .padding()
            }
            .frame(width: 400, height: 250)
        }
    }
}

private struct StepTwoVerify: View {
    @ObservedObject var store: DeskdropStore
    var selectedPeer: PeerViewModel?
    var onCancel: () -> Void
    
    @State private var hasTimedOut = false
    
    var body: some View {
        VStack(spacing: 24) {
            Text("Step 2: Verify & Trust")
                .font(.system(size: 28, weight: .bold, design: .rounded))
            
            if let peer = selectedPeer {
                if let pin = peer.pairingPin {
                    Text("Ensure this matches the code on \(peer.displayName):")
                        .foregroundStyle(CRTheme.inkSoft)
                    
                    Text(pin)
                        .font(.system(size: 32, weight: .black, design: .monospaced))
                        .tracking(8)
                        .padding()
                        .background(CRTheme.surfaceElevated)
                        .cornerRadius(12)
                    
                    if peer.pairingRequested {
                        HStack(spacing: 16) {
                            Button("Decline") {
                                store.respondToPairing(ManagedDevice(peer: peer), accepted: false)
                            }
                            .buttonStyle(CRSecondaryButtonStyle())
                            
                            Button("Trust Device") {
                                store.respondToPairing(ManagedDevice(peer: peer), accepted: true)
                            }
                            .buttonStyle(CRPrimaryButtonStyle(tint: CRTheme.accentGreen))
                        }
                    }
                } else if hasTimedOut {
                    Text("Connection failed or timed out.")
                        .foregroundStyle(CRTheme.accentRed)
                    
                    Button("Try Again") {
                        onCancel()
                    }
                    .buttonStyle(CRSecondaryButtonStyle())
                } else {
                    Text("Connecting to \(peer.displayName)...")
                        .foregroundStyle(CRTheme.inkSoft)
                    ProgressView()
                }
            } else {
                Text("No device selected.")
            }
        }
        .onAppear {
            DispatchQueue.main.asyncAfter(deadline: .now() + 10.0) {
                if selectedPeer?.pairingPin == nil {
                    hasTimedOut = true
                }
            }
        }
        .onChange(of: selectedPeer?.pairingPin) { _ in
            hasTimedOut = false
        }
    }
}


private struct RadarPulseView: View {
    @State private var isPulsing = false
    
    var body: some View {
        ZStack {
            Circle()
                .fill(CRTheme.brandElectric.opacity(0.1))
                .frame(width: 70, height: 70)
                .scaleEffect(isPulsing ? 1.8 : 0.5)
                .opacity(isPulsing ? 0 : 1)
            
            Circle()
                .fill(CRTheme.brandElectric.opacity(0.2))
                .frame(width: 50, height: 50)
                .scaleEffect(isPulsing ? 1.5 : 0.8)
                .opacity(isPulsing ? 0 : 1)
            
            if #available(macOS 14.0, *) {
                Image(systemName: "antenna.radiowaves.left.and.right")
                    .font(.system(size: 28, weight: .semibold))
                    .foregroundStyle(CRTheme.brandElectric)
                    .symbolEffect(.variableColor.cumulative, options: .repeating)
            } else {
                Image(systemName: "antenna.radiowaves.left.and.right")
                    .font(.system(size: 28, weight: .semibold))
                    .foregroundStyle(CRTheme.brandElectric)
            }
        }
        .onAppear {
            withAnimation(.easeOut(duration: 2.0).repeatForever(autoreverses: false)) {
                isPulsing = true
            }
        }
    }
}
