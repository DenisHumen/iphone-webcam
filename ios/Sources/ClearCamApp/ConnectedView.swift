#if canImport(UIKit)
    import ClearCamCore
    import SwiftUI

    struct ConnectedView: View {
        @ObservedObject var vm: ConnectViewModel
        let payload: QRPayload

        var body: some View {
            VStack(spacing: 16) {
                Text("ClearCam")
                    .font(.largeTitle.bold())
                StatusLine(state: vm.state)
                VStack(alignment: .leading, spacing: 4) {
                    row("Хост", payload.host)
                    row("Control", "\(payload.cport)")
                    row("Media", "\(payload.mport)")
                    if let sid = vm.sessionId {
                        row("Session", String(sid.prefix(12)) + "…")
                    }
                    if let t = vm.lastTelemetry {
                        row("Батарея", "\(Int(t.batteryLevel * 100))%")
                    }
                }
                .padding()
                .background(.secondary.opacity(0.1))
                .clipShape(RoundedRectangle(cornerRadius: 12))
                Spacer()
                Button(role: .destructive) {
                    Task { await vm.disconnect() }
                } label: {
                    Text("Отключиться")
                        .frame(maxWidth: .infinity)
                        .padding()
                }
                .buttonStyle(.borderedProminent)
            }
            .padding()
        }

        @ViewBuilder private func row(_ k: String, _ v: String) -> some View {
            HStack {
                Text(k).foregroundStyle(.secondary)
                Spacer()
                Text(v).font(.body.monospaced())
            }
        }
    }

    private struct StatusLine: View {
        let state: SessionState
        var body: some View {
            HStack(spacing: 8) {
                Circle().fill(color).frame(width: 8, height: 8)
                Text(label).font(.subheadline)
            }
        }
        private var label: String {
            switch state {
            case .idle: return "Не запущено"
            case .connecting: return "Подключение…"
            case .handshaking: return "Handshake…"
            case .ready: return "Подключено"
            case .reconnecting: return "Переподключение…"
            case .stopped(let r): return "Завершено: \(r)"
            }
        }
        private var color: Color {
            switch state {
            case .ready: return .green
            case .reconnecting, .handshaking, .connecting: return .yellow
            case .stopped: return .red
            case .idle: return .gray
            }
        }
    }
#endif
