import XCTest
@testable import ClearCamCore

final class ReconnectTests: XCTestCase {
    func testFirstAttemptCloseToBase() {
        let r = Reconnect(base: .milliseconds(500), cap: .seconds(15))
        for _ in 0..<32 {
            let d = r.delay(forAttempt: 0)
            let ms = Double(d.components.seconds) * 1000 + Double(d.components.attoseconds) / 1e15
            XCTAssertGreaterThanOrEqual(ms, 500 * 0.85)
            XCTAssertLessThanOrEqual(ms, 500 * 1.15)
        }
    }

    func testCappedAtCap() {
        let r = Reconnect(base: .milliseconds(500), cap: .seconds(2))
        let d = r.delay(forAttempt: 30)
        let ms = Double(d.components.seconds) * 1000 + Double(d.components.attoseconds) / 1e15
        XCTAssertLessThanOrEqual(ms, 2000 * 1.15 + 1)
    }

    func testGrowsExponentially() {
        let r = Reconnect(base: .milliseconds(500), cap: .seconds(60), factor: 2.0)
        let d0 = r.delay(forAttempt: 0)
        let d3 = r.delay(forAttempt: 3)
        let ms0 =
            Double(d0.components.seconds) * 1000 + Double(d0.components.attoseconds) / 1e15
        let ms3 =
            Double(d3.components.seconds) * 1000 + Double(d3.components.attoseconds) / 1e15
        // d3 should be ~4–8x d0 (factor^3 = 8 with ±15% jitter).
        XCTAssertGreaterThan(ms3, ms0 * 3)
    }
}
