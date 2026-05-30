import ClearCamProtocol
import Foundation
import XCTest
@testable import ClearCamCore

final class ControlStreamTests: XCTestCase {
    func testRoundTripEnvelope() async throws {
        let pipe = MemoryPipe()
        let client = ControlStream(io: pipe.endA)
        let server = ControlStream(io: pipe.endB)

        let env = ControlEnvelope(
            seq: 7,
            ack: nil,
            body: .auth(Auth.token("secret"))
        )
        try await client.send(env)
        let got = try await server.recv()
        XCTAssertEqual(got, env)
    }

    func testMultipleEnvelopesInOrder() async throws {
        let pipe = MemoryPipe()
        let client = ControlStream(io: pipe.endA)
        let server = ControlStream(io: pipe.endB)

        let envs: [ControlEnvelope] = [
            ControlEnvelope(seq: 1, body: .auth(Auth.token("a"))),
            ControlEnvelope(seq: 2, body: .bye(Bye(reason: "hi"))),
            ControlEnvelope(seq: 3, body: .ping(Ping(tsUsec: 123))),
        ]
        for e in envs { try await client.send(e) }
        for e in envs {
            let got = try await server.recv()
            XCTAssertEqual(got, e)
        }
    }

    func testCancelPropagates() async throws {
        let pipe = MemoryPipe()
        let server = ControlStream(io: pipe.endB)
        await server.cancel()
        do {
            _ = try await server.recv()
            XCTFail("expected recv to fail after cancel")
        } catch {
            // ok
        }
    }
}

/// Two `RawIO`-shaped endpoints sharing in-memory byte queues.
final class MemoryPipe: @unchecked Sendable {
    let endA: PipeEnd
    let endB: PipeEnd

    init() {
        let aToB = ByteQueue()
        let bToA = ByteQueue()
        endA = PipeEnd(out: aToB, in: bToA)
        endB = PipeEnd(out: bToA, in: aToB)
    }
}

final class PipeEnd: RawIO, @unchecked Sendable {
    private let outQueue: ByteQueue
    private let inQueue: ByteQueue

    init(out: ByteQueue, in inQ: ByteQueue) {
        outQueue = out
        inQueue = inQ
    }

    func sendData(_ data: Data) async throws {
        await outQueue.write(data)
    }

    func receiveExactly(_ n: Int) async throws -> Data {
        try await inQueue.readExactly(n)
    }

    func cancel() {
        Task { await outQueue.close() }
        Task { await inQueue.close() }
    }
}

/// In-memory byte queue with async readers/writers and close semantics.
actor ByteQueue {
    private var buffer = Data()
    private var waiter: (Int, CheckedContinuation<Data, Error>)?
    private var closed = false

    func write(_ data: Data) {
        buffer.append(data)
        tryServeWaiter()
    }

    func readExactly(_ n: Int) async throws -> Data {
        if buffer.count >= n {
            let chunk = buffer.prefix(n)
            buffer.removeFirst(n)
            return chunk
        }
        if closed {
            throw ControlStreamError.truncated
        }
        return try await withCheckedThrowingContinuation { cont in
            waiter = (n, cont)
            tryServeWaiter()
        }
    }

    func close() {
        closed = true
        if let (_, cont) = waiter {
            waiter = nil
            cont.resume(throwing: ControlStreamError.cancelled)
        }
    }

    private func tryServeWaiter() {
        guard let (n, cont) = waiter else { return }
        if buffer.count >= n {
            waiter = nil
            let chunk = buffer.prefix(n)
            buffer.removeFirst(n)
            cont.resume(returning: chunk)
        } else if closed {
            waiter = nil
            cont.resume(throwing: ControlStreamError.truncated)
        }
    }
}
