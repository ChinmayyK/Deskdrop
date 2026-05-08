import SwiftUI

struct CommandPaletteView: View {
    @ObservedObject var store: ClipRelayStore
    @State private var query = ""
    @FocusState private var focused: Bool

    var suggestions: [String] {
        let all = [
            "/history",
            "/devices",
            "/send \(store.connectedDevices.first?.name ?? "<device>")",
            "/connect 192.168.1.20:47823",
            "/trust \(store.devices.first?.name ?? "<device>")",
        ]

        if query.isEmpty { return all }
        return all.filter { $0.localizedCaseInsensitiveContains(query) }
    }

    var body: some View {
        PBPanel(dark: true) {
            VStack(alignment: .leading, spacing: 16) {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Command Palette")
                        .font(.system(size: 25, weight: .bold, design: .serif))
                        .foregroundStyle(.white)
                    Text("Jump directly to history, devices, trust actions, and manual connects.")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(.white.opacity(0.64))
                }

                HStack(spacing: 10) {
                    Image(systemName: "command")
                        .foregroundStyle(.white.opacity(0.66))
                    TextField("Run a command", text: $query)
                        .textFieldStyle(.plain)
                        .focused($focused)
                        .onSubmit {
                            store.performCommand(query)
                        }
                }
                .pbInput(dark: true)

                VStack(spacing: 10) {
                    ForEach(suggestions, id: \.self) { suggestion in
                        Button {
                            query = suggestion
                            store.performCommand(suggestion)
                        } label: {
                            HStack {
                                Text(suggestion)
                                    .font(.system(size: 14, weight: .medium, design: .monospaced))
                                    .foregroundStyle(.white)
                                Spacer()
                                Image(systemName: "arrow.up.left")
                                    .font(.system(size: 11, weight: .bold))
                                    .foregroundStyle(.white.opacity(0.45))
                            }
                            .padding(14)
                            .background(
                                RoundedRectangle(cornerRadius: 16, style: .continuous)
                                    .fill(Color.white.opacity(0.08))
                                    .overlay(
                                        RoundedRectangle(cornerRadius: 16, style: .continuous)
                                            .stroke(Color.white.opacity(0.08), lineWidth: 1)
                                    )
                            )
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            .padding(18)
        }
        .frame(width: 540)
        .onAppear { focused = true }
    }
}
