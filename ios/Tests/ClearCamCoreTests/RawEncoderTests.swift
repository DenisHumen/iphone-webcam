import ClearCamProtocol
import Foundation
import XCTest
@testable import ClearCamCore

final class RawEncoderTests: XCTestCase {
    func testPacksTightlyAndProducesValidHeader() throws {
        let w: UInt16 = 16
        let h: UInt16 = 16
        let y = Data(repeating: 100, count: Int(w) * Int(h))
        let uv = Data(repeating: 128, count: Int(w) * Int(h) / 2)
        let packed = try RawEncoder.packNV12(
            y: y, uv: uv, width: w, height: h, seq: 7, ptsUsec: 1234, fullRange: true)
        XCTAssertEqual(packed.header.width, w)
        XCTAssertEqual(packed.header.height, h)
        XCTAssertEqual(packed.header.seq, 7)
        XCTAssertEqual(packed.header.ptsUsec, 1234)
        XCTAssertEqual(packed.header.payloadLen, UInt32(Int(w) * Int(h) * 3 / 2))
        XCTAssertEqual(packed.header.codec, .raw)
        XCTAssertTrue(packed.header.flags.contains(.fullRange))
        XCTAssertEqual(packed.payload.count, 384)
    }

    func testWireBytesRoundTrip() throws {
        let w: UInt16 = 8
        let h: UInt16 = 8
        let y = Data(repeating: 0xAA, count: 64)
        let uv = Data(repeating: 0xBB, count: 32)
        let packed = try RawEncoder.packNV12(y: y, uv: uv, width: w, height: h, seq: 1, ptsUsec: 0)
        let wire = packed.wireBytes()
        // 28-byte header + 96-byte payload.
        XCTAssertEqual(wire.count, 28 + 96)
        // Round-trip the header from the leading 28 bytes and confirm fields.
        let parsed = try MediaHeader.decode(from: wire.prefix(28))
        XCTAssertEqual(parsed, packed.header)
        // First Y byte after header is the start of the payload.
        XCTAssertEqual(wire[28], 0xAA)
        // First UV byte at offset 28 + 64.
        XCTAssertEqual(wire[28 + 64], 0xBB)
    }

    func testRejectsPlaneSizeMismatch() {
        let y = Data(repeating: 0, count: 100)
        let uv = Data(repeating: 0, count: 50)
        XCTAssertThrowsError(
            try RawEncoder.packNV12(y: y, uv: uv, width: 16, height: 16, seq: 0, ptsUsec: 0))
    }
}
