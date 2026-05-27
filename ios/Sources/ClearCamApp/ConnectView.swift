#if canImport(UIKit)
    import ClearCamCore
    import SwiftUI

    public struct ConnectView: View {
        @StateObject private var vm: ConnectViewModel

        public init(vm: ConnectViewModel) {
            _vm = StateObject(wrappedValue: vm)
        }

        public var body: some View {
            ZStack {
                if let payload = vm.payload {
                    ConnectedView(vm: vm, payload: payload)
                } else if vm.useManual {
                    ManualEntry(vm: vm)
                } else {
                    QRScannerView { data in
                        Task { await vm.acceptScannedPayload(data) }
                    }
                    .ignoresSafeArea()
                    .overlay(alignment: .bottom) {
                        Button("Ввести вручную") { vm.useManual = true }
                            .padding()
                            .background(.white.opacity(0.9))
                            .clipShape(Capsule())
                            .padding(.bottom, 40)
                    }
                }
                if let err = vm.error {
                    VStack {
                        Spacer()
                        Text(err)
                            .padding()
                            .background(Color.red.opacity(0.85))
                            .foregroundStyle(.white)
                            .clipShape(RoundedRectangle(cornerRadius: 12))
                            .padding()
                    }
                }
            }
        }
    }

    private struct ManualEntry: View {
        @ObservedObject var vm: ConnectViewModel
        @State private var host = ""
        @State private var cport = ""
        @State private var mport = ""
        @State private var token = ""

        var body: some View {
            Form {
                Section("Подключение") {
                    TextField("Host (IP)", text: $host)
                        .keyboardType(.numbersAndPunctuation)
                        .autocapitalization(.none)
                    TextField("Control port", text: $cport).keyboardType(.numberPad)
                    TextField("Media port", text: $mport).keyboardType(.numberPad)
                    TextField("Token", text: $token)
                }
                Button("Подключиться") {
                    Task {
                        await vm.acceptManual(
                            host: host, cport: Int(cport) ?? 0, mport: Int(mport) ?? 0,
                            token: token)
                    }
                }
                .disabled(host.isEmpty || cport.isEmpty || mport.isEmpty || token.isEmpty)
                Button("Назад к QR") { vm.useManual = false }
            }
        }
    }
#endif
