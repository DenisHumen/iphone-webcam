import XCTest
@testable import ClearCamCore

final class QRPayloadTests: XCTestCase {
    func testDecodesDesktopPayload() throws {
        let json = #"""
        {"v":1,"host":"192.168.1.42","cport":7000,"mport":7001,"token":"abc"}
        """#
        let p = try QRPayload.decode(json)
        XCTAssertEqual(p.v, 1)
        XCTAssertEqual(p.host, "192.168.1.42")
        XCTAssertEqual(p.cport, 7000)
        XCTAssertEqual(p.mport, 7001)
        XCTAssertEqual(p.token, "abc")
    }

    func testRejectsWrongVersion() {
        let json = #"{"v":999,"host":"a","cport":1,"mport":2,"token":"t"}"#
        XCTAssertThrowsError(try QRPayload.decode(json)) { err in
            guard case QRPayload.DecodeError.unsupportedVersion(let v) = err else {
                XCTFail("expected unsupportedVersion, got \(err)")
                return
            }
            XCTAssertEqual(v, 999)
        }
    }

    func testRejectsMissingFields() {
        let json = #"{"v":1,"host":"","cport":7000,"mport":7001,"token":"abc"}"#
        XCTAssertThrowsError(try QRPayload.decode(json))
    }

    func testRoundTripEncode() throws {
        let p = QRPayload(v: 1, host: "10.0.0.5", cport: 9000, mport: 9001, token: "tok")
        let data = try JSONEncoder().encode(p)
        let back = try QRPayload.decode(data)
        XCTAssertEqual(back, p)
    }
}
