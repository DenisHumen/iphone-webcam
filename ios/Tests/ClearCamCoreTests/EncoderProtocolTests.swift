import ClearCamProtocol
import Foundation
import XCTest
@testable import ClearCamCore

final class EncoderProtocolTests: XCTestCase {
    func testStubEmitsConfigOnceThenFrames() async throws {
        let mode = EncoderMode(
            codec: .hevc, width: 1280, height: 720, fps: 30, bitrateKbps: 12_000)
        let encoder = StubEncoder(mode: mode)

        var emitted: [EncodedAU] = []
        try await encoder.encode(1, ptsUsec: 1000) { au in
            emitted.append(au)
        }
        // First call: config + frame
        XCTAssertEqual(emitted.count, 2)
        XCTAssertTrue(emitted[0].isConfig)
        XCTAssertFalse(emitted[0].isKeyframe)
        XCTAssertEqual(emitted[0].nalUnits.count, 3) // VPS/SPS/PPS
        XCTAssertTrue(emitted[1].isKeyframe) // first frame keyframe

        emitted.removeAll()
        for i in 2...30 {
            try await encoder.encode(UInt32(i), ptsUsec: UInt64(i) * 33_000) { au in
                emitted.append(au)
            }
        }
        XCTAssertEqual(emitted.count, 29)
        let keys = emitted.filter { $0.isKeyframe }
        XCTAssertEqual(keys.count, 0) // next keyframe lands on call 31
    }

    func testWireBodyIsLengthPrefixedNals() {
        let au = EncodedAU(
            isKeyframe: true,
            isConfig: false,
            nalUnits: [Data([0xAA, 0xBB]), Data([0xCC])],
            ptsUsec: 0)
        let body = au.wireBody()
        // [0,0,0,2][AA BB][0,0,0,1][CC]
        XCTAssertEqual(
            Array(body),
            [0, 0, 0, 2, 0xAA, 0xBB, 0, 0, 0, 1, 0xCC])
    }
}
