import SwiftUI

struct RootContainerView: View {
    @ObservedObject var store: DeskdropStore
    @AppStorage("hasCompletedOnboarding") private var hasCompletedOnboarding = false

    var body: some View {
        Group {
            if hasCompletedOnboarding {
                DashboardRootView(store: store)
            } else {
                OnboardingView(store: store, onComplete: {
                    hasCompletedOnboarding = true
                })
            }
        }
        .frame(minWidth: 1020, minHeight: 700)
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
        if let lastSync = peer.lastSync, lastSync > sessionStartTime {
            return 3
        }
        return 2
    }

    let onComplete: () -> Void

    var body: some View {
        ZStack {
            CRFluidBackgroundView().ignoresSafeArea()

            VStack(spacing: 0) {
                // Header (Pagination)
                HStack(spacing: 8) {
                    ForEach(0..<4) { step in
                        Circle()
                            .fill(step == currentStep ? CRTheme.brandElectric : CRTheme.strokeSoft)
                            .frame(width: 8, height: 8)
                            .scaleEffect(step == currentStep ? 1.2 : 1.0)
                            .animation(.crSpring, value: currentStep)
                    }
                }
                .padding(.top, 40)
                
                Spacer()

                ZStack {
                    if currentStep == 0 {
                        StepOneFindDevice(store: store, selectedPeerId: $selectedPeerId)
                            .transition(.asymmetric(insertion: .move(edge: .trailing).combined(with: .opacity), removal: .move(edge: .leading).combined(with: .opacity)))
                    } else if currentStep == 1 {
                        StepTwoVerify(store: store, selectedPeer: selectedPeer, onCancel: { selectedPeerId = nil })
                            .transition(.asymmetric(insertion: .move(edge: .trailing).combined(with: .opacity), removal: .move(edge: .leading).combined(with: .opacity)))
                    } else if currentStep == 2 {
                        StepThreeSendSample(store: store, selectedPeer: selectedPeer)
                            .transition(.asymmetric(insertion: .move(edge: .trailing).combined(with: .opacity), removal: .move(edge: .leading).combined(with: .opacity)))
                    } else if currentStep == 3 {
                        StepFourCompletion(onComplete: onComplete)
                            .transition(.asymmetric(insertion: .move(edge: .trailing).combined(with: .opacity), removal: .move(edge: .leading).combined(with: .opacity)))
                    }
                }
                .animation(.crSpring, value: currentStep)
                
                Spacer()
                
                // Footer Navigation
                HStack {
                    if currentStep == 0 {
                        Button("Skip for now") {
                            onComplete()
                        }
                        .buttonStyle(CRSecondaryButtonStyle())
                    } else if currentStep > 0 {
                        Button("Cancel") {
                            withAnimation(.crSpring) { selectedPeerId = nil }
                        }
                        .buttonStyle(CRSecondaryButtonStyle())
                    } else {
                        Spacer().frame(width: 80)
                    }
                    
                    Spacer()
                    
                    if currentStep == 3 {
                        Button("Get Started") {
                            onComplete()
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
                        Text("Searching for nearby devices...").foregroundStyle(CRTheme.inkSoft).padding()
                    } else {
                        ForEach(store.peers) { peer in
                            Button {
                                selectedPeerId = peer.id
                                store.connectAndPair(deviceId: peer.id)
                            } label: {
                                HStack {
                                    Image(systemName: peer.displayName.lowercased().contains("mac") ? "laptopcomputer" : "smartphone")
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

private struct StepThreeSendSample: View {
    @ObservedObject var store: DeskdropStore
    var selectedPeer: PeerViewModel?
    
    var body: some View {
        VStack(spacing: 24) {
            Text("Step 3: Send Sample Text")
                .font(.system(size: 28, weight: .bold, design: .rounded))
            
            Text("Let's make sure it works. Click the button below to send 'Hello from Mac' to your device.")
                .foregroundStyle(CRTheme.inkSoft)
                .multilineTextAlignment(.center)
                .frame(width: 400)
            
            Button("Send 'Hello from Mac'") {
                if let peer = selectedPeer {
                    store.sendPushText("Hello from Mac", to: ManagedDevice(peer: peer))
                }
            }
            .buttonStyle(CRPrimaryButtonStyle(tint: CRTheme.brandElectric))
        }
    }
}

private struct StepFourCompletion: View {
    var onComplete: () -> Void
    
    var body: some View {
        VStack(spacing: 24) {
            ZStack {
                Circle().fill(CRTheme.accentGreen.opacity(0.1))
                    .frame(width: 100, height: 100)
                Image(systemName: "checkmark")
                    .font(.system(size: 40, weight: .bold))
                    .foregroundStyle(CRTheme.accentGreen)
            }
            
            Text("You're all set!")
                .font(.system(size: 28, weight: .bold, design: .rounded))
            
            Text("Received files will automatically land in your Downloads folder.\nClipboard text will be instantly available to paste.")
                .foregroundStyle(CRTheme.inkSoft)
                .multilineTextAlignment(.center)
                .lineSpacing(4)
        }
    }
}
