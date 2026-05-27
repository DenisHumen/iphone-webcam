import XCTest

@testable import ClearCamProtocol

final class ControlEnvelopeFramingTests: XCTestCase {
    func testFrameWritesBELengthPrefix() {
        let framed = ControlFraming.frame(Data("abc".utf8))
        XCTAssertEqual(Array(framed.prefix(4)), [0, 0, 0, 3])
        XCTAssertEqual(framed.suffix(3), Data("abc".utf8))
    }

    func testFrameEmpty() {
        XCTAssertEqual(Array(ControlFraming.frame(Data())), [0, 0, 0, 0])
    }

    func testRoundTrip() throws {
        let payload = Data(#"{"t":"HELLO","seq":1}"#.utf8)
        let framed = ControlFraming.frame(payload)
        guard let (got, n) = try ControlFraming.tryUnframe(framed) else {
            XCTFail("no frame")
            return
        }
        XCTAssertEqual(got, payload)
        XCTAssertEqual(n, framed.count)
    }

    func testPartial() throws {
        let buf = Data([0, 0, 0, 5, UInt8(ascii: "h"), UInt8(ascii: "e")])
        XCTAssertNil(try ControlFraming.tryUnframe(buf))
    }

    func testTooLarge() {
        let buf = Data([0xFF, 0xFF, 0xFF, 0xFF])
        XCTAssertThrowsError(try ControlFraming.tryUnframe(buf, maxPayload: 1024)) { err in
            XCTAssertEqual(err as? ControlFraming.FrameError, .tooLarge(len: .max, max: 1024))
        }
    }
}
