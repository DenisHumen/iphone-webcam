#if canImport(UIKit)
    import AVFoundation
    import ClearCamCore
    import ClearCamProtocol
    import CoreVideo
    import Foundation
    import SwiftUI

    @MainActor
    public final class ConnectViewModel: ObservableObject {
        @Published public var state: SessionState = .idle
        @Published public var payload: QRPayload?
        @Published public var sessionId: String?
        @Published public var lastTelemetry: Telemetry?
        @Published public var error: String?
        @Published public var useManual: Bool = false

        private let infoProvider: any DeviceInfoProvider
        private var controller: SessionController?
        private var telemetryEmitter: TelemetryEmitter?
        private var captureEngine: AVCaptureEngine?

        public init(infoProvider: any DeviceInfoProvider = UIKitDeviceInfo()) {
            self.infoProvider = infoProvider
        }

        public func acceptScannedPayload(_ data: Data) async {
            do {
                let p = try QRPayload.decode(data)
                await connect(payload: p)
            } catch {
                self.error = "QR: \(error)"
            }
        }

        public func acceptManual(host: String, cport: Int, mport: Int, token: String) async {
            let p = QRPayload(
                v: QRPayload.supportedVersion, host: host, cport: cport, mport: mport, token: token)
            await connect(payload: p)
        }

        public func disconnect() async {
            await telemetryEmitter?.stop()
            telemetryEmitter = nil
            await controller?.disconnect(reason: "user")
            controller = nil
            captureEngine = nil
            sessionId = nil
            payload = nil
            state = .idle
            lastTelemetry = nil
        }

        private func connect(payload: QRPayload) async {
            self.payload = payload
            self.error = nil
            state = .connecting
            do {
                let client = try TransportClient(
                    host: payload.host, cport: payload.cport, mport: payload.mport)
                let (control, media) = try await client.connect()
                let controller = SessionController(
                    controlChannel: control, mediaChannel: media, deviceInfo: infoProvider)
                self.controller = controller
                state = .handshaking
                let sid = try await controller.connect(token: payload.token)
                sessionId = sid
                state = .ready(sessionId: sid)
                try await wireCamera(controller: controller)
                startTelemetry(controller: controller)
            } catch {
                self.error = "Ошибка: \(error)"
                state = .stopped(reason: "\(error)")
            }
        }

        /// Construct an AVCaptureEngine whose onFrame callback packs the
        /// CVPixelBuffer and writes it via `controller.sendCapturedNV12`, then
        /// hand the engine + discovery to the controller and publish
        /// CAMERA_LIST.
        private func wireCamera(controller: SessionController) async throws {
            let engine = AVCaptureEngine { [weak controller] pixelBuffer, pts in
                guard let controller else { return }
                let ptsUsec = UInt64(max(0, CMTimeGetSeconds(pts)) * 1_000_000)
                guard
                    let packed = try? CVPixelBufferPacker.packNV12(
                        pixelBuffer, seq: 0, ptsUsec: ptsUsec)
                else { return }
                let y = packed.payload.prefix(Int(packed.header.width) * Int(packed.header.height))
                let uv = packed.payload.suffix(
                    Int(packed.header.width) * Int(packed.header.height) / 2)
                Task { [weak controller] in
                    try? await controller?.sendCapturedNV12(
                        y: Data(y),
                        uv: Data(uv),
                        width: packed.header.width,
                        height: packed.header.height,
                        ptsUsec: ptsUsec,
                        fullRange: true)
                }
            }
            captureEngine = engine
            await controller.attachCameraSystem(engine: engine, discovery: AVCameraDiscovery())
            try await controller.publishCameraListAndStart()
        }

        private func startTelemetry(controller: SessionController) {
            let emitter = TelemetryEmitter()
            self.telemetryEmitter = emitter
            let provider = infoProvider
            Task { [weak self] in
                await emitter.start(info: provider) { [weak self, weak controller] t in
                    if let controller {
                        try? await controller.sendTelemetry(t)
                    }
                    await MainActor.run {
                        self?.lastTelemetry = t
                    }
                }
            }
        }
    }
#endif
