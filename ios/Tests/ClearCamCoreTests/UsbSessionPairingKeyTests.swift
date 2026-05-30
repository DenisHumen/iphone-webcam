import ClearCamProtocol
import Foundation
import XCTest
@testable import ClearCamCore

// MARK: - Resolver-level tests (PairingStore + TrustController invariants)

@MainActor
final class UsbSessionPairingKeyTests: XCTestCase {

    // -------------------------------------------------------------------------
    // Test 1: Stored PairingKey is returned by the store and its base64NoPad
    // form is non-empty and contains no trailing "=" padding, pinning the
    // wire-format invariant: AUTH.token == key.base64NoPad.
    // -------------------------------------------------------------------------
    func testStoredPairingKeyIsUsedAsAuth() async throws {
        let store = PairingStore(provider: .inMemory)
        let key = PairingKey.random()
        try await store.put(udid: "PC-1", key: key)

        let stored = try await store.get(udid: "PC-1")
        XCTAssertEqual(stored?.bytes, key.bytes)
        XCTAssertFalse(key.base64NoPad.isEmpty)
        // The wire token must not carry base-64 padding.
        XCTAssertFalse(key.base64NoPad.hasSuffix("="))

        // AuthCredential.pairingKey wraps the key; wireToken must equal base64NoPad.
        let cred = AuthCredential.pairingKey(key)
        XCTAssertEqual(cred.wireToken, key.base64NoPad)
    }

    // -------------------------------------------------------------------------
    // Test 2: Unknown peer surfaces a TrustPrompt; accepting it produces an
    // accepted outcome with a non-empty key.
    // -------------------------------------------------------------------------
    func testUnknownPeerPromptsTrustController() async throws {
        let store = PairingStore(provider: .inMemory)
        let trust = TrustController(store: store)

        let outcome = Task<TrustOutcome, Never> {
            await trust.requestTrust(forDesktopId: "PC-2")
        }
        await Task.yield()
        XCTAssertNotNil(trust.pendingPrompt)
        XCTAssertEqual(trust.pendingPrompt?.desktopId, "PC-2")
        trust.respond(accept: true)
        let result = await outcome.value
        guard case .accepted(let key) = result else {
            return XCTFail("expected accepted")
        }
        XCTAssertFalse(key.base64NoPad.isEmpty)
        XCTAssertFalse(key.base64NoPad.hasSuffix("="))
    }

    // -------------------------------------------------------------------------
    // Test 3: Wire-level integration — SessionController.connect(credential:)
    // using a PairingKey sends an AUTH envelope whose token equals the key's
    // base64NoPad representation.
    // -------------------------------------------------------------------------
    func testConnectWithPairingKeyWritesCorrectAuthToken() async throws {
        let key = PairingKey.random()
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

        // Server: respond to HELLO with HELLO_ACK, then capture the AUTH token.
        var capturedToken: String?
        async let serverWork: String = {
            let hello = try await serverControl.recv()
            guard case .hello = hello.body else {
                XCTFail("expected HELLO"); return ""
            }
            try await serverControl.send(
                ControlEnvelope(
                    seq: 0, ack: hello.seq,
                    body: .helloAck(HelloAck(protoVer: ClearCamProtocolVersion.current, caps: []))))

            let authEnv = try await serverControl.recv()
            guard case .auth(let auth) = authEnv.body else {
                XCTFail("expected AUTH, got \(authEnv.body)"); return ""
            }
            if case .pairingKey(let pk) = auth {
                capturedToken = pk
            } else if case .token(let t) = auth {
                capturedToken = t
            }

            try await serverControl.send(
                ControlEnvelope(
                    seq: 1, ack: authEnv.seq,
                    body: .authOk(AuthOk(sessionId: "usb-sess-1"))))
            // Drain MEDIA_HELLO and DEVICE_INFO so client doesn't block.
            _ = try await serverMedia.recv()
            _ = try await serverControl.recv()
            return "usb-sess-1"
        }()

        let sid = try await controller.connect(credential: .pairingKey(key))
        let serverSid = try await serverWork

        XCTAssertEqual(sid, "usb-sess-1")
        XCTAssertEqual(serverSid, "usb-sess-1")
        XCTAssertEqual(capturedToken, key.base64NoPad,
            "AUTH.token on the wire must equal key.base64NoPad")
    }
}
