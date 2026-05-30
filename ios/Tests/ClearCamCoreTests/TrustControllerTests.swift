import XCTest
@testable import ClearCamCore

@MainActor
final class TrustControllerTests: XCTestCase {
    func testUnpairedDevicePromptsUntilAccept() async throws {
        let store = PairingStore(provider: .inMemory)
        let controller = TrustController(store: store)
        let outcome = Task<TrustOutcome, Never> {
            await controller.requestTrust(forDesktopId: "DESKTOP-1")
        }
        await Task.yield()
        XCTAssertEqual(controller.pendingPrompt?.desktopId, "DESKTOP-1")
        controller.respond(accept: true)
        let result = await outcome.value
        guard case .accepted(let key) = result else { return XCTFail("expected accepted") }
        // Allow the actor's `Task { await store.put(...) }` (started inside
        // respond) to complete before reading the store.
        await Task.yield()
        try await Task.sleep(nanoseconds: 50_000_000)
        let stored = try await store.get(udid: "DESKTOP-1")
        XCTAssertEqual(stored?.bytes, key.bytes)
    }

    func testRejectionReturnsDeniedAndStoresNothing() async throws {
        let store = PairingStore(provider: .inMemory)
        let controller = TrustController(store: store)
        let outcome = Task<TrustOutcome, Never> {
            await controller.requestTrust(forDesktopId: "DESKTOP-2")
        }
        await Task.yield()
        controller.respond(accept: false)
        let result = await outcome.value
        XCTAssertEqual(result, .denied)
        let stored = try await store.get(udid: "DESKTOP-2")
        XCTAssertNil(stored)
    }

    func testRespondBeforeRequestIsANoOp() {
        let store = PairingStore(provider: .inMemory)
        let controller = TrustController(store: store)
        // No outstanding request — respond is a no-op (does not crash).
        controller.respond(accept: true)
        XCTAssertNil(controller.pendingPrompt)
    }
}
