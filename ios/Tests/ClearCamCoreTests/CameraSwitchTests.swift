import ClearCamProtocol
import Foundation
import XCTest
@testable import ClearCamCore

final class CameraSwitchTests: XCTestCase {
    func testRespondsToSetCameraWithCameraState() async throws {
        let controlPipe = MemoryPipe()
        let mediaPipe = MemoryPipe()

        let clientControl = ControlStream(io: controlPipe.endA)
        let clientMedia = ControlStream(io: mediaPipe.endA)
        let serverControl = ControlStream(io: controlPipe.endB)
        let serverMedia = ControlStream(io: mediaPipe.endB)

        let info = StubInfoProvider()
        let discovery = StubCameraDiscovery([
            CameraEntry(
                id: "wide", name: "Wide", position: .back, maxWidth: 4032, maxHeight: 3024,
                maxFps: 60, supportedFormats: ["nv12"]),
            CameraEntry(
                id: "tele", name: "Telephoto", position: .back, maxWidth: 4032, maxHeight: 3024,
                maxFps: 60, supportedFormats: ["nv12"]),
        ])
        let engine = RecordingCaptureEngine()
        let controller = SessionController(
            controlChannel: clientControl,
            mediaChannel: clientMedia,
            deviceInfo: info,
            cameraDiscovery: discovery,
            captureEngine: engine)

        // Server: respond to HELLO + AUTH; consume MEDIA_HELLO, DEVICE_INFO, CAMERA_LIST.
        async let serverHandshake: Void = {
            let hello = try await serverControl.recv()
            try await serverControl.send(
                ControlEnvelope(
                    seq: 0, ack: hello.seq,
                    body: .helloAck(
                        HelloAck(protoVer: ClearCamProtocolVersion.current, caps: []))))
            let auth = try await serverControl.recv()
            try await serverControl.send(
                ControlEnvelope(
                    seq: 1, ack: auth.seq,
                    body: .authOk(AuthOk(sessionId: "sess-camera"))))
            let mediaHello = try await serverMedia.recv()
            guard case .mediaHello = mediaHello.body else { return }
            let di = try await serverControl.recv()
            guard case .deviceInfo = di.body else { return }
            let cl = try await serverControl.recv()
            guard case .cameraList(let list) = cl.body else { return }
            XCTAssertEqual(list.cameras.count, 2)
        }()

        let sid = try await controller.connect(token: "secret")
        XCTAssertEqual(sid, "sess-camera")
        try await serverHandshake

        // Confirm the engine started on the first back camera ("wide").
        let started = await engine.startedCameras
        XCTAssertEqual(started, ["wide"])

        // Now the server sends SET_CAMERA(tele). The pump must apply it and
        // emit CAMERA_STATE(activeCameraId="tele").
        try await serverControl.send(
            ControlEnvelope(
                seq: 99, ack: nil,
                body: .setCamera(SetCamera(cameraId: "tele"))))
        let confirmation = try await serverControl.recv()
        guard case .cameraState(let cs) = confirmation.body else {
            XCTFail("expected CAMERA_STATE, got \(confirmation.body)")
            return
        }
        XCTAssertEqual(cs.activeCameraId, "tele")
        XCTAssertEqual(cs.appliedFormat, "raw nv12")
        XCTAssertEqual(confirmation.ack, 99)
        let switched = await engine.switchedCameras
        XCTAssertEqual(switched, ["tele"])

        await controller.disconnect(reason: "test-done")
    }
}

/// Records every call so the test can assert without AVFoundation.
final actor RecordingCaptureEngine: CaptureEngine {
    var startedCameras: [String] = []
    var switchedCameras: [String] = []
    nonisolated init() {}
    func start(cameraId: String) async throws {
        startedCameras.append(cameraId)
    }
    func setCamera(_ cameraId: String) async throws {
        switchedCameras.append(cameraId)
    }
    func stop() async {}
}
