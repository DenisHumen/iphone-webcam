import ClearCamProtocol
import Foundation

public enum SessionError: Error, Equatable, Sendable {
    case unexpectedMessage(String)
    case helloAckMissing
    case authFailed(String)
    case transport(String)
}

/// Drives the iPhone-side handshake state machine: HELLO → AUTH → MEDIA_HELLO.
/// On success, lands in `.ready` and continues to write DEVICE_INFO + telemetry
/// until `disconnect()` or an I/O failure.
public actor SessionController {
    public private(set) var state: SessionState = .idle
    public private(set) var sessionId: String?

    private let controlChannel: ControlStream
    private let mediaChannel: ControlStream
    private let deviceInfo: any DeviceInfoProvider
    private let appName: String

    private var seq: UInt64 = 0

    public init(
        controlChannel: ControlStream,
        mediaChannel: ControlStream,
        deviceInfo: any DeviceInfoProvider,
        appName: String = "ClearCam-iOS/0.1.0"
    ) {
        self.controlChannel = controlChannel
        self.mediaChannel = mediaChannel
        self.deviceInfo = deviceInfo
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
        let sessionId: String
        switch authResp.body {
        case .authOk(let ok):
            sessionId = ok.sessionId
        case .error(let e):
            state = .stopped(reason: "auth: \(e.code.rawValue)")
            throw SessionError.authFailed(e.message)
        default:
            state = .stopped(reason: "unexpected reply")
            throw SessionError.unexpectedMessage("expected AUTH_OK")
        }
        self.sessionId = sessionId

        // Media handshake — separate channel.
        try await mediaChannel.send(
            ControlEnvelope(
                seq: 0, ack: nil,
                body: .mediaHello(MediaHello(sessionId: sessionId, token: token))))

        // DEVICE_INFO on control.
        seq += 1
        try await controlChannel.send(
            ControlEnvelope(seq: seq, ack: nil, body: .deviceInfo(snap)))

        state = .ready(sessionId: sessionId)
        return sessionId
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
        seq += 1
        _ = try? await controlChannel.send(
            ControlEnvelope(seq: seq, ack: nil, body: .bye(Bye(reason: reason))))
        await controlChannel.cancel()
        await mediaChannel.cancel()
        state = .stopped(reason: reason)
    }
}
