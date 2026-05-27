import XCTest

@testable import ClearCamProtocol

final class MediaHeaderTests: XCTestCase {
    func sample() -> MediaHeader {
        MediaHeader(
            mediaType: .video,
            flags: [.keyframe, .encoded, .fullRange],
            codec: .hevc,
            width: 1920, height: 1080,
            seq: 42, ptsUsec: 1_234_567_890,
            payloadLen: 12345
        )
    }

    /// Same 28 bytes as the Rust `known_bytes_encode` test in `ccp_protocol::media`.
    /// If you change one side, you MUST update the other.
    func testKnownBytesEncode() {
        let h = sample()
        let bytes = h.encode()
        let expected: [UInt8] = [
            0xCC, 0x01, 0x01, 0x07, 0x01, 0x00, 0x00, 0x00,
            0x07, 0x80, 0x04, 0x38,
            0x00, 0x00, 0x00, 0x2A,
            0x00, 0x00, 0x00, 0x00, 0x49, 0x96, 0x02, 0xD2,
            0x00, 0x00, 0x30, 0x39,
        ]
        XCTAssertEqual(Array(bytes), expected)
    }

    func testRoundTrip() throws {
        let h = sample()
        let back = try MediaHeader.decode(from: h.encode())
        XCTAssertEqual(back, h)
    }

    func testRoundTripZero() throws {
        let h = MediaHeader(
            mediaType: .video, flags: [], codec: .raw,
            width: 0, height: 0, seq: 0, ptsUsec: 0, payloadLen: 0
        )
        let back = try MediaHeader.decode(from: h.encode())
        XCTAssertEqual(back, h)
    }

    func testTooShort() {
        do {
            _ = try MediaHeader.decode(from: Data(count: MediaConstants.headerLen - 1))
            XCTFail("expected error")
        } catch let MediaHeaderError.tooShort(got) {
            XCTAssertEqual(got, MediaConstants.headerLen - 1)
        } catch {
            XCTFail("wrong error: \(error)")
        }
    }

    func testBadMagic() {
        var b = sample().encode()
        b[0] = 0
        XCTAssertThrowsError(try MediaHeader.decode(from: b)) { err in
            if case .badMagic = err as? MediaHeaderError {} else {
                XCTFail("wrong: \(err)")
            }
        }
    }

    func testUnknownCodec() {
        var b = sample().encode()
        b[4] = 99
        XCTAssertThrowsError(try MediaHeader.decode(from: b)) { err in
            XCTAssertEqual(err as? MediaHeaderError, .unknownCodec(99))
        }
    }

    func testUnknownMediaType() {
        var b = sample().encode()
        b[2] = 99
        XCTAssertThrowsError(try MediaHeader.decode(from: b)) { err in
            XCTAssertEqual(err as? MediaHeaderError, .unknownMediaType(99))
        }
    }

    func testReservedIgnored() throws {
        var b = sample().encode()
        b[5] = 0xAB; b[6] = 0xCD; b[7] = 0xEF
        XCTAssertEqual(try MediaHeader.decode(from: b), sample())
    }

    func testUnknownFlagsMasked() throws {
        var b = sample().encode()
        b[3] = 0xFF
        let h = try MediaHeader.decode(from: b)
        XCTAssertEqual(h.flags, MediaFlags.known)
    }
}
