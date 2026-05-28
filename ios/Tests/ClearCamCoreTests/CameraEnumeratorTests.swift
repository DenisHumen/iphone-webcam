import ClearCamProtocol
import XCTest
@testable import ClearCamCore

final class CameraEnumeratorTests: XCTestCase {
    func testStubReturnsConfiguredEntries() {
        let entries = [
            CameraEntry(
                id: "wide", name: "Wide", position: .back, maxWidth: 4032, maxHeight: 3024,
                maxFps: 60, supportedFormats: ["nv12"]),
            CameraEntry(
                id: "tele", name: "Telephoto", position: .back, maxWidth: 4032, maxHeight: 3024,
                maxFps: 60, supportedFormats: ["nv12"]),
        ]
        let d = StubCameraDiscovery(entries)
        XCTAssertEqual(d.discover().count, 2)
        XCTAssertEqual(d.discover()[1].id, "tele")
    }
}
