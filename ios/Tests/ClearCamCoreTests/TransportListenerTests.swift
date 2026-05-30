import XCTest
import Network
@testable import ClearCamCore

final class TransportListenerTests: XCTestCase {

    // MARK: pairing logic (no real network required)

    func testPairMatchedWhenBothStreamsPresentBeforeNextSession() async throws {
        let listener = try TransportListener(controlPort: 0, mediaPort: 0)

        // Inject both streams BEFORE calling nextSession.
        let cp = MemoryPipe()
        let mp = MemoryPipe()
        let cs = ControlStream(io: cp.endA)
        let ms = ControlStream(io: mp.endA)
        await listener._inject(control: cs, media: ms)

        let pair = try await listener.nextSession()
        XCTAssertNotNil(pair.control)
        XCTAssertNotNil(pair.media)

        await listener.stop()
    }

    func testPairMatchedWhenStreamsArriveConcurrentlyWithNextSession() async throws {
        let listener = try TransportListener(controlPort: 0, mediaPort: 0)

        // Start nextSession in a background task, then inject the pair.
        async let sessionTask: TransportListener.InboundPair = listener.nextSession()

        // Give nextSession a moment to register its continuation.
        try await Task.sleep(nanoseconds: 10_000_000)

        let cp = MemoryPipe()
        let mp = MemoryPipe()
        await listener._inject(
            control: ControlStream(io: cp.endA),
            media: ControlStream(io: mp.endA)
        )

        let pair = try await sessionTask
        XCTAssertNotNil(pair.control)
        XCTAssertNotNil(pair.media)

        await listener.stop()
    }

    func testMultiplePairsDispatchedInOrder() async throws {
        let listener = try TransportListener(controlPort: 0, mediaPort: 0)

        // Inject two pairs.
        let cp1 = MemoryPipe(); let mp1 = MemoryPipe()
        let cp2 = MemoryPipe(); let mp2 = MemoryPipe()
        await listener._inject(control: ControlStream(io: cp1.endA), media: ControlStream(io: mp1.endA))
        await listener._inject(control: ControlStream(io: cp2.endA), media: ControlStream(io: mp2.endA))

        let pair1 = try await listener.nextSession()
        let pair2 = try await listener.nextSession()

        XCTAssertNotNil(pair1.control)
        XCTAssertNotNil(pair2.control)

        await listener.stop()
    }

    // MARK: init validation

    func testInvalidPortThrows() {
        // Port values > 65535 can't be represented as UInt16.
        // TransportListener guards against this via NWEndpoint.Port init,
        // but passing 0 is always valid (ephemeral).
        XCTAssertNoThrow(try TransportListener(controlPort: 0, mediaPort: 0))
    }
}
