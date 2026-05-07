import SwiftUI

struct QuickAccessHistoryView: View {
    @ObservedObject var store: ClipRelayStore
    @State private var search = ""

    private var results: [TimelineItem] {
        if search.isEmpty { return store.timeline }
        return store.timeline.filter {
            $0.title.localizedCaseInsensitiveContains(search) ||
            $0.sourceDevice.localizedCaseInsensitiveContains(search)
        }
    }

    var body: some View {
        PBPanel {
            VStack(spacing: 16) {
                HStack(alignment: .center, spacing: 12) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Quick Access")
                            .font(.system(size: 25, weight: .bold, design: .serif))
                            .foregroundStyle(PBTheme.ink)
                        Text("Search, copy, or resend your recent clipboard items.")
                            .font(.system(size: 13, weight: .medium))
                            .foregroundStyle(PBTheme.inkSoft)
                    }

                    Spacer()

                    HStack(spacing: 10) {
                        Image(systemName: "magnifyingglass")
                            .foregroundStyle(PBTheme.inkSoft)
                        TextField("Search clipboard history", text: $search)
                            .textFieldStyle(.plain)
                    }
                    .pbInput()
                    .frame(width: 220)
                }

                ScrollView {
                    LazyVStack(spacing: 10) {
                        if let context = store.quickSendContext, !context.text.isEmpty {
                            QuickSendStripView(store: store, text: context.text)
                        }

                        if results.isEmpty {
                            VStack(spacing: 10) {
                                Image(systemName: "doc.text.magnifyingglass")
                                    .font(.system(size: 30))
                                    .foregroundStyle(PBTheme.accentBlue)
                                Text(search.isEmpty ? "No items yet" : "Nothing matched")
                                    .font(.system(size: 16, weight: .semibold))
                                    .foregroundStyle(PBTheme.ink)
                                Text(search.isEmpty ? "Recent clipboard history will appear here." : "Try a shorter or different search phrase.")
                                    .font(.system(size: 13, weight: .medium))
                                    .foregroundStyle(PBTheme.inkSoft)
                            }
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 28)
                        } else {
                            ForEach(results.prefix(25)) { item in
                                QuickHistoryRow(item: item, store: store)
                            }
                        }
                    }
                    .padding(.vertical, 2)
                }
            }
            .padding(18)
        }
        .frame(width: 460, height: 540)
    }
}

private struct QuickSendStripView: View {
    @ObservedObject var store: ClipRelayStore
    let text: String

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                PBBadge("JUST COPIED", tint: PBTheme.accentBlue)
                Spacer()
                Text("\(store.connectedDevices.count) target\(store.connectedDevices.count == 1 ? "" : "s")")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(PBTheme.inkSoft)
            }

            Text(text)
                .font(.system(size: 16, weight: .semibold))
                .foregroundStyle(PBTheme.ink)
                .lineLimit(3)

            HStack(spacing: 8) {
                Button("Send to all") { store.sendCurrentClipboard(to: nil) }
                    .buttonStyle(PBPrimaryButtonStyle())
                ForEach(store.connectedDevices.prefix(3)) { device in
                    Button(device.name) { store.sendCurrentClipboard(to: device) }
                        .buttonStyle(PBSecondaryButtonStyle())
                }
            }
        }
        .padding(18)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 22, style: .continuous)
                .fill(
                    LinearGradient(
                        colors: [PBTheme.accentBlue.opacity(0.12), Color.white],
                        startPoint: .topLeading,
                        endPoint: .bottomTrailing
                    )
                )
                .overlay(
                    RoundedRectangle(cornerRadius: 22, style: .continuous)
                        .stroke(PBTheme.accentBlue.opacity(0.18), lineWidth: 1)
                )
        )
    }
}

private struct QuickHistoryRow: View {
    let item: TimelineItem
    @ObservedObject var store: ClipRelayStore

    var body: some View {
        Button {
            store.copyTimelineItem(item)
        } label: {
            HStack(alignment: .top, spacing: 12) {
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .fill(PBTheme.accentBlue.opacity(0.12))
                    .frame(width: 34, height: 34)
                    .overlay(
                        Image(systemName: item.iconName)
                            .font(.system(size: 14, weight: .bold))
                            .foregroundStyle(PBTheme.accentBlue)
                    )

                VStack(alignment: .leading, spacing: 6) {
                    Text(item.title)
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundStyle(PBTheme.ink)
                        .lineLimit(2)
                    HStack(spacing: 6) {
                        Text(item.sourceDevice)
                        Text("•")
                        Text(item.timestamp.relativeTimeString())
                    }
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(PBTheme.inkSoft)
                }

                Spacer()
            }
            .padding(14)
            .background(
                RoundedRectangle(cornerRadius: 18, style: .continuous)
                    .fill(PBTheme.surfaceStrong)
                    .overlay(
                        RoundedRectangle(cornerRadius: 18, style: .continuous)
                            .stroke(PBTheme.stroke, lineWidth: 1)
                    )
            )
        }
        .buttonStyle(.plain)
        .contextMenu {
            Button("Copy to this Mac") { store.copyTimelineItem(item) }
            Menu("Send to device") {
                Button("Send to all devices") { store.sendTimelineItem(item, to: nil) }
                ForEach(store.connectedDevices) { device in
                    Button(device.name) { store.sendTimelineItem(item, to: device) }
                }
            }
            Button(item.pinned ? "Unpin" : "Pin") {
                store.pinTimelineItem(item, pinned: !item.pinned)
            }
            Button("Delete", role: .destructive) {
                store.deleteTimelineItem(item)
            }
        }
    }
}
