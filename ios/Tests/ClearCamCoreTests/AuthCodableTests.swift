import XCTest
@testable import ClearCamProtocol

final class AuthCodableTests: XCTestCase {
    func testTokenEncodesWithTokenField() throws {
        let auth = Auth.token("abc")
        let data = try JSONEncoder().encode(auth)
        let json = String(data: data, encoding: .utf8)!
        XCTAssertEqual(json, #"{"token":"abc"}"#)
    }

    func testPairingKeyEncodesWithCamelCaseField() throws {
        let auth = Auth.pairingKey("KEY-B64")
        let data = try JSONEncoder().encode(auth)
        let json = String(data: data, encoding: .utf8)!
        XCTAssertEqual(json, #"{"pairingKey":"KEY-B64"}"#)
    }

    func testTokenDecodes() throws {
        let auth = try JSONDecoder().decode(Auth.self, from: Data(#"{"token":"x"}"#.utf8))
        XCTAssertEqual(auth, .token("x"))
    }

    func testPairingKeyDecodes() throws {
        let auth = try JSONDecoder().decode(Auth.self, from: Data(#"{"pairingKey":"k"}"#.utf8))
        XCTAssertEqual(auth, .pairingKey("k"))
    }
}
