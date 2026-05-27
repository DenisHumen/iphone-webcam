import ClearCamProtocol
import Foundation
import XCTest
@testable import ClearCamCore

final class SessionControllerTests: XCTestCase {
    func testHandshakeReachesReady() async throws {
        let controlPipe = MemoryPipe()
        let mediaPipe = MemoryPipe()

        let clientControl = ControlStream(io: controlPipe.endA)
        let clientMedia = ControlStream(io: mediaPipe.endA)
        let serverControl = ControlStream(io: controlPipe.endB)
        let serverMedia = ControlStream(io: mediaPipe.endB)

        let info = StubInfoProvider()
        let controller = SessionController(
            controlChannel: clientControl,
            mediaChannel: clientMedia,
            deviceInfo: info)

        // Server-side: respond to HELLO and AUTH.
        async let serverWork: String = {
            let hello = try await serverControl.recv()
            guard case .hello = hello.body else {
                XCTFail("expected HELLO, got \(hello.body)")
                return ""
            }
            try await serverControl.send(
                ControlEnvelope(
                    seq: 0, ack: hello.seq,
                    body: .helloAck(
                        HelloAck(protoVer: ClearCamProtocolVersion.current, caps: []))))
            let auth = try await serverControl.recv()
            guard case .auth = auth.body else {
                XCTFail("expected AUTH, got \(auth.body)")
                return ""
            }
            try await serverControl.send(
                ControlEnvelope(
                    seq: 1, ack: auth.seq,
                    body: .authOk(AuthOk(sessionId: "sess-test"))))
            // Drain MEDIA_HELLO and DEVICE_INFO so client doesn't block.
            let mediaHello = try await serverMedia.recv()
            guard case .mediaHello = mediaHello.body else {
                XCTFail("expected MEDIA_HELLO, got \(mediaHello.body)")
                return ""
            }
            let di = try await serverControl.recv()
            guard case .deviceInfo = di.body else {
                XCTFail("expected DEVICE_INFO, got \(di.body)")
                return ""
            }
            return "sess-test"
        }()

        let returned = try await controller.connect(token: "secret")
        XCTAssertEqual(returned, "sess-test")
        let serverSessionId = try await serverWork
        XCTAssertEqual(serverSessionId, "sess-test")
        let st = await controller.state
        if case .ready(let sid) = st {
            XCTAssertEqual(sid, "sess-test")
        } else {
            XCTFail("expected .ready, got \(st)")
        }
    }

    func testRejectsServerError() async throws {
        let controlPipe = MemoryPipe()
        let mediaPipe = MemoryPipe()

        let clientControl = ControlStream(io: controlPipe.endA)
        let clientMedia = ControlStream(io: mediaPipe.endA)
        let serverControl = ControlStream(io: controlPipe.endB)

        let info = StubInfoProvider()
        let controller = SessionController(
            controlChannel: clientControl,
            mediaChannel: clientMedia,
            deviceInfo: info)

        async let serverWork: Void = {
            let hello = try await serverControl.recv()
            try await serverControl.send(
                ControlEnvelope(
                    seq: 0, ack: hello.seq,
                    body: .error(
                        ErrorMsg(code: .incompatibleVersion, message: "version mismatch"))))
        }()

        do {
            _ = try await controller.connect(token: "secret")
            XCTFail("expected SessionError")
        } catch SessionError.authFailed(let msg) {
            XCTAssertEqual(msg, "version mismatch")
        }
        try await serverWork
    }
}

struct StubInfoProvider: DeviceInfoProvider {
    var snapshot: DeviceInfo {
        DeviceInfo(
            model: "iPhone15,3",
            osVer: "iOS 18.0",
            batteryLevel: 0.9,
            batteryState: .unplugged,
            thermalState: .nominal,
            usb3Capable: true)
    }
}
