import XCTest
@testable import ClearCamCore

final class PairingStoreTests: XCTestCase {
    func testRoundTripPutGet() async throws {
        let store = PairingStore(provider: .inMemory)
        let key = PairingKey.random()
        try await store.put(udid: "UDID-A", key: key)
        let got = try await store.get(udid: "UDID-A")
        XCTAssertEqual(got?.bytes, key.bytes)
    }

    func testForgetRemoves() async throws {
        let store = PairingStore(provider: .inMemory)
        try await store.put(udid: "U", key: .random())
        try await store.forget(udid: "U")
        let got = try await store.get(udid: "U")
        XCTAssertNil(got)
    }

    func testGetReturnsNilForUnknownUdid() async throws {
        let store = PairingStore(provider: .inMemory)
        let got = try await store.get(udid: "missing")
        XCTAssertNil(got)
    }

    func testPairingKeyRandomIs32Bytes() {
        let k = PairingKey.random()
        XCTAssertEqual(k.bytes.count, 32)
    }

    func testBase64NoPadOmitsPadding() {
        let k = PairingKey(Data([0])) // single zero byte → "AA==" with padding; we want "AA"
        XCTAssertEqual(k.base64NoPad, "AA")
    }
}
