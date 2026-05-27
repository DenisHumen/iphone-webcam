import XCTest

@testable import ClearCamProtocol

final class ControlMessageJSONTests: XCTestCase {
    func testHelloEnvelope() throws {
        let env = ControlEnvelope(
            seq: 1, ack: nil,
            body: .hello(
                Hello(
                    protoVer: 1,
                    app: "ClearCam-iOS/0.1.0",
                    device: DeviceIdent(model: "iPhone15,3", osVer: "iOS 18.0"),
                    sessionId: "sess-abc",
                    caps: [.hevc, .rawNv12]
                )
            )
        )
        let bytes = try env.encodeJSON()
        let obj = try JSONSerialization.jsonObject(with: bytes) as? [String: Any]
        XCTAssertEqual(obj?["t"] as? String, "HELLO")
        XCTAssertEqual(obj?["seq"] as? Int, 1)
        XCTAssertEqual(obj?["protoVer"] as? Int, 1)
        XCTAssertNil(obj?["ack"])

        let back = try ControlEnvelope.decodeJSON(bytes)
        XCTAssertEqual(back, env)
    }

    func testErrorEnvelopeWithAck() throws {
        let env = ControlEnvelope(
            seq: 100, ack: 99,
            body: .error(ErrorMsg(code: .badRequest, message: "oops"))
        )
        let bytes = try env.encodeJSON()
        let obj = try JSONSerialization.jsonObject(with: bytes) as? [String: Any]
        XCTAssertEqual(obj?["ack"] as? Int, 99)
        XCTAssertEqual(obj?["code"] as? String, "bad_request")
        let back = try ControlEnvelope.decodeJSON(bytes)
        XCTAssertEqual(back, env)
    }

    func testAllVariantsRoundTrip() throws {
        let mode = Mode(
            format: .raw, codec: .none, width: 1280, height: 720, fps: 30,
            bitrateKbps: 0, pixelFormat: .nv12, fullRange: false
        )

        let bodies: [ControlMessage] = [
            .hello(
                Hello(
                    protoVer: 1, app: "x",
                    device: DeviceIdent(model: "m", osVer: "v"),
                    sessionId: "s", caps: [.hevc])),
            .helloAck(HelloAck(protoVer: 1, caps: [])),
            .auth(Auth(token: "t")),
            .authOk(AuthOk(sessionId: "s")),
            .mediaHello(MediaHello(sessionId: "s", token: "t")),
            .bye(Bye(reason: "done")),
            .error(ErrorMsg(code: .timeout, message: "x")),
            .deviceInfo(
                DeviceInfo(
                    model: "m", osVer: "v", batteryLevel: 0.5,
                    batteryState: .charging, thermalState: .nominal,
                    usb3Capable: false)),
            .cameraList(CameraList(cameras: [])),
            .setCamera(SetCamera(cameraId: "wide")),
            .cameraState(CameraState(activeCameraId: "wide", appliedFormat: "1080p30 hevc")),
            .start(Start(mode: mode)),
            .stop(Stop()),
            .setMode(SetMode(mode: mode)),
            .modeApplied(ModeApplied(mode: mode, atSeq: 0)),
            .telemetry(
                Telemetry(
                    tsUsec: 0, batteryLevel: 0,
                    batteryState: .unknown, thermalState: .nominal,
                    sentBitrateKbps: 0, encFps: 0, captureFps: 0,
                    queueDepth: 0, dropCount: 0)),
            .ping(Ping(tsUsec: 0)),
            .pong(Pong(tsUsec: 0, echoUsec: 0)),
            .speedtestStart(
                SpeedtestStart(
                    id: "x", targetBitrateKbps: 0, durationMs: 0, pattern: .ramp)),
            .speedtestTick(SpeedtestTick(id: "x", tickIndex: 0, tsUsec: 0)),
            .speedtestResult(
                SpeedtestResult(
                    id: "x", goodputMbps: 0, rttMs: 0, jitterMs: 0, lossPct: 0,
                    recommendedMode: mode)),
        ]

        for body in bodies {
            let env = ControlEnvelope(seq: 1, body: body)
            let bytes = try env.encodeJSON()
            let back = try ControlEnvelope.decodeJSON(bytes)
            XCTAssertEqual(back, env, "round-trip failed for \(body.typeTag)")
        }
    }
}
