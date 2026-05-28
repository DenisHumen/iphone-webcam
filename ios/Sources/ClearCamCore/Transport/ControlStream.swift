import ClearCamProtocol
import Foundation
import Network

public enum ControlStreamError: Error, Sendable {
    case io(String)
    case truncated
    case oversized(UInt32)
    case decode(String)
    case cancelled
}

/// Lowest-level byte channel `ControlStream` reads/writes against.
///
/// Production is backed by `NWConnection` (TCP). Tests can swap in an in-memory
/// pipe to verify framing without touching the OS network stack.
public protocol RawIO: Sendable {
    func sendData(_ data: Data) async throws
    func receiveExactly(_ n: Int) async throws -> Data
    func cancel()
}

/// Length-prefixed JSON envelope reader/writer over a `RawIO` transport.
///
/// Wire layout: `[uint32 BE length][UTF-8 JSON payload]` per docs/03 §5.1.
/// The actor serializes access to the underlying channel.
public actor ControlStream {
    public static let maxPayload: UInt32 = ControlFraming.defaultMaxPayload

    private let io: any RawIO

    public init(io: any RawIO) {
        self.io = io
    }

    public func send(_ env: ControlEnvelope) async throws {
        let payload = try env.encodeJSON()
        guard payload.count <= Int(Self.maxPayload) else {
            throw ControlStreamError.oversized(UInt32(payload.count))
        }
        let frame = ControlFraming.frame(payload)
        try await io.sendData(frame)
    }

    public func recv() async throws -> ControlEnvelope {
        let prefix = try await io.receiveExactly(4)
        let i = prefix.startIndex
        let len =
            (UInt32(prefix[i]) << 24)
            | (UInt32(prefix[i + 1]) << 16)
            | (UInt32(prefix[i + 2]) << 8)
            | UInt32(prefix[i + 3])
        guard len <= Self.maxPayload else { throw ControlStreamError.oversized(len) }
        let body = len == 0 ? Data() : try await io.receiveExactly(Int(len))
        do {
            return try ControlEnvelope.decodeJSON(body)
        } catch {
            throw ControlStreamError.decode("\(error)")
        }
    }

    /// Write raw bytes onto the underlying socket without length prefixing.
    /// Used for media frames after MEDIA_HELLO completes — each call writes a
    /// `[28-byte MediaHeader][payload]` record per docs/03 §5.2.
    public func sendRaw(_ data: Data) async throws {
        try await io.sendData(data)
    }

    public func cancel() {
        io.cancel()
    }
}

/// `RawIO` impl over `NWConnection`. Use in production iOS app and integration
/// tests that talk to a real desktop server.
public final class NWConnectionIO: RawIO {
    private let connection: NWConnection

    public init(connection: NWConnection) {
        self.connection = connection
    }

    public func sendData(_ data: Data) async throws {
        try await withCheckedThrowingContinuation {
            (cont: CheckedContinuation<Void, Error>) in
            connection.send(
                content: data,
                completion: .contentProcessed { err in
                    if let err {
                        cont.resume(throwing: ControlStreamError.io(err.localizedDescription))
                    } else {
                        cont.resume()
                    }
                })
        }
    }

    public func receiveExactly(_ n: Int) async throws -> Data {
        guard n > 0 else { return Data() }
        return try await withCheckedThrowingContinuation {
            (cont: CheckedContinuation<Data, Error>) in
            connection.receive(minimumIncompleteLength: n, maximumLength: n) {
                data, _, isComplete, err in
                if let err {
                    cont.resume(throwing: ControlStreamError.io(err.localizedDescription))
                    return
                }
                if let data, data.count == n {
                    cont.resume(returning: data)
                    return
                }
                if isComplete {
                    cont.resume(throwing: ControlStreamError.truncated)
                    return
                }
                cont.resume(throwing: ControlStreamError.truncated)
            }
        }
    }

    public func cancel() {
        connection.cancel()
    }
}
