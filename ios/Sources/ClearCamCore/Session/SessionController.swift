import ClearCamProtocol
import Foundation

public enum SessionError: Error, Equatable, Sendable {
    case unexpectedMessage(String)
    case helloAckMissing
    case authFailed(String)
    case transport(String)
}

/// Drives the iPhone-side handshake state machine: HELLO → AUTH → MEDIA_HELLO.
/// On success, lands in `.ready` and continues to write DEVICE_INFO,
/// CAMERA_LIST, and (Phase 2) reacts to SET_CAMERA envelopes from the desktop.
public actor SessionController {
    public private(set) var state: SessionState = .idle
    public private(set) var sessionId: String?
    public private(set) var activeCameraId: String?

    private let controlChannel: ControlStream
    private let mediaChannel: ControlStream
    private let deviceInfo: any DeviceInfoProvider
    private let cameraDiscovery: (any CameraDiscovery)?
    private let captureEngine: (any CaptureEngine)?
    private let appName: String

    private var seq: UInt64 = 0
    private var pumpTask: Task<Void, Never>?

    public init(
        controlChannel: ControlStream,
        mediaChannel: ControlStream,
        deviceInfo: any DeviceInfoProvider,
        cameraDiscovery: (any CameraDiscovery)? = nil,
        captureEngine: (any CaptureEngine)? = nil,
        appName: String = "ClearCam-iOS/0.1.0"
    ) {
        self.controlChannel = controlChannel
        self.mediaChannel = mediaChannel
        self.deviceInfo = deviceInfo
        self.cameraDiscovery = cameraDiscovery
        self.captureEngine = captureEngine
        self.appName = appName
    }

    public func observeState(_ block: @Sendable (SessionState) -> Void) {
        block(state)
    }

    /// Drive the handshake to `.ready`. Returns the assigned `sessionId` on success.
    @discardableResult
    public func connect(token: String, sessionCandidate: String = UUID().uuidString) async throws
        -> String
    {
        state = .connecting
        state = .handshaking

        let snap = deviceInfo.snapshot
        seq += 1
        try await controlChannel.send(
            ControlEnvelope(
                seq: seq,
                ack: nil,
                body: .hello(
                    Hello(
                        protoVer: ClearCamProtocolVersion.current,
                        app: appName,
                        device: DeviceIdent(model: snap.model, osVer: snap.osVer),
                        sessionId: sessionCandidate,
                        caps: [.hevc, .h264, .rawNv12]
                    ))
            ))

        let ack: ControlEnvelope
        do {
            ack = try await controlChannel.recv()
        } catch {
            state = .stopped(reason: "transport: \(error)")
            throw SessionError.transport("\(error)")
        }
        switch ack.body {
        case .helloAck:
            break
        case .error(let e):
            state = .stopped(reason: "server: \(e.code.rawValue)")
            throw SessionError.authFailed(e.message)
        default:
            state = .stopped(reason: "unexpected reply")
            throw SessionError.unexpectedMessage("expected HELLO_ACK")
        }

        seq += 1
        try await controlChannel.send(
            ControlEnvelope(seq: seq, ack: nil, body: .auth(Auth(token: token))))
        let authResp = try await controlChannel.recv()
        let sid: String
        switch authResp.body {
        case .authOk(let ok):
            sid = ok.sessionId
        case .error(let e):
            state = .stopped(reason: "auth: \(e.code.rawValue)")
            throw SessionError.authFailed(e.message)
        default:
            state = .stopped(reason: "unexpected reply")
            throw SessionError.unexpectedMessage("expected AUTH_OK")
        }
        sessionId = sid

        // Media handshake — separate channel.
        try await mediaChannel.send(
            ControlEnvelope(
                seq: 0, ack: nil,
                body: .mediaHello(MediaHello(sessionId: sid, token: token))))

        // DEVICE_INFO on control.
        seq += 1
        try await controlChannel.send(
            ControlEnvelope(seq: seq, ack: nil, body: .deviceInfo(snap)))

        // CAMERA_LIST (if a discovery is wired) + auto-start the first camera.
        if let discovery = cameraDiscovery {
            let cameras = discovery.discover()
            seq += 1
            try await controlChannel.send(
                ControlEnvelope(
                    seq: seq, ack: nil,
                    body: .cameraList(CameraList(cameras: cameras))))
            let first = cameras.first(where: { $0.position == .back }) ?? cameras.first
            if let first, let engine = captureEngine {
                try await engine.start(cameraId: first.id)
                activeCameraId = first.id
            }
        }

        state = .ready(sessionId: sid)
        startPump()
        return sid
    }

    /// Begin draining inbound control envelopes after handshake; reacts to
    /// PING and SET_CAMERA. Other types are ignored in Phase 2.
    private func startPump() {
        pumpTask?.cancel()
        pumpTask = Task { [weak self] in
            guard let self else { return }
            while !Task.isCancelled {
                let env: ControlEnvelope
                do {
                    env = try await self.controlChannel.recv()
                } catch {
                    return
                }
                switch env.body {
                case .ping(let p):
                    try? await self.handlePing(seq: env.seq, tsUsec: p.tsUsec)
                case .setCamera(let sc):
                    try? await self.applySetCamera(seq: env.seq, cameraId: sc.cameraId)
                case .bye:
                    return
                default:
                    continue
                }
            }
        }
    }

    private func applySetCamera(seq peerSeq: UInt64, cameraId: String) async throws {
        if let engine = captureEngine {
            try await engine.setCamera(cameraId)
        }
        activeCameraId = cameraId
        seq += 1
        try await controlChannel.send(
            ControlEnvelope(
                seq: seq, ack: peerSeq,
                body: .cameraState(
                    CameraState(activeCameraId: cameraId, appliedFormat: "raw nv12"))))
    }

    /// Emit a TELEMETRY envelope. Caller drives the schedule.
    public func sendTelemetry(_ t: Telemetry) async throws {
        seq += 1
        try await controlChannel.send(
            ControlEnvelope(seq: seq, ack: nil, body: .telemetry(t)))
    }

    /// Reply to a PING from the desktop.
    public func handlePing(seq peerSeq: UInt64, tsUsec: UInt64) async throws {
        self.seq += 1
        try await controlChannel.send(
            ControlEnvelope(
                seq: self.seq, ack: peerSeq,
                body: .pong(
                    Pong(
                        tsUsec: UInt64(Date().timeIntervalSince1970 * 1_000_000),
                        echoUsec: tsUsec))))
    }

    /// Send BYE and close streams.
    public func disconnect(reason: String) async {
        pumpTask?.cancel()
        pumpTask = nil
        seq += 1
        _ = try? await controlChannel.send(
            ControlEnvelope(seq: seq, ack: nil, body: .bye(Bye(reason: reason))))
        await controlChannel.cancel()
        await mediaChannel.cancel()
        if let engine = captureEngine {
            await engine.stop()
        }
        state = .stopped(reason: reason)
    }
}
