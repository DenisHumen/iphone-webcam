import XCTest
@testable import ClearCamProtocol

final class ModeCodableTests: XCTestCase {
    func testEncodedHEVC1080p30RoundTrips() throws {
        let mode = Mode(
            format: .encoded, codec: .hevc,
            width: 1920, height: 1080, fps: 30,
            bitrateKbps: 30_000, pixelFormat: .nv12, fullRange: true
        )
        let data = try JSONEncoder().encode(mode)
        let back = try JSONDecoder().decode(Mode.self, from: data)
        XCTAssertEqual(back, mode)
    }

    func testRawPath() throws {
        let mode = Mode(
            format: .raw, codec: .none,
            width: 1280, height: 720, fps: 30,
            bitrateKbps: 0, pixelFormat: .nv12, fullRange: true
        )
        let data = try JSONEncoder().encode(mode)
        let back = try JSONDecoder().decode(Mode.self, from: data)
        XCTAssertEqual(back, mode)
    }

    func testCamelCaseWireFormat() throws {
        let mode = Mode(
            format: .encoded, codec: .hevc,
            width: 1920, height: 1080, fps: 30,
            bitrateKbps: 30_000, pixelFormat: .nv12, fullRange: true
        )
        let data = try JSONEncoder().encode(mode)
        let json = String(data: data, encoding: .utf8)!
        XCTAssertTrue(json.contains("\"bitrateKbps\":30000"),
            "expected camelCase bitrateKbps, got: \(json)")
        XCTAssertTrue(json.contains("\"pixelFormat\":\"nv12\""))
        XCTAssertTrue(json.contains("\"fullRange\":true"))
    }
}
