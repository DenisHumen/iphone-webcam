import Foundation
import Network

public enum TransportClientError: Error, Sendable {
    case invalidPort
    case io(String)
    case cancelled
}

/// Opens both control and media TCP connections to the desktop server.
public final class TransportClient: @unchecked Sendable {
    public let host: NWEndpoint.Host
    public let controlPort: NWEndpoint.Port
    public let mediaPort: NWEndpoint.Port

    public init(host: String, cport: Int, mport: Int) throws {
        guard let cp = NWEndpoint.Port(rawValue: UInt16(cport)),
            let mp = NWEndpoint.Port(rawValue: UInt16(mport))
        else {
            throw TransportClientError.invalidPort
        }
        self.host = NWEndpoint.Host(host)
        self.controlPort = cp
        self.mediaPort = mp
    }

    public func connect() async throws -> (ControlStream, ControlStream) {
        let control = try await openTCP(port: controlPort)
        let media = try await openTCP(port: mediaPort)
        return (
            ControlStream(io: NWConnectionIO(connection: control)),
            ControlStream(io: NWConnectionIO(connection: media))
        )
    }

    private func openTCP(port: NWEndpoint.Port) async throws -> NWConnection {
        let conn = NWConnection(host: host, port: port, using: .tcp)
        try await withCheckedThrowingContinuation {
            (cont: CheckedContinuation<Void, Error>) in
            let resumed = LockedBool(false)
            conn.stateUpdateHandler = { state in
                switch state {
                case .ready:
                    if resumed.swap(true) == false { cont.resume() }
                case .failed(let err):
                    if resumed.swap(true) == false {
                        cont.resume(throwing: TransportClientError.io(err.localizedDescription))
                    }
                case .cancelled:
                    if resumed.swap(true) == false {
                        cont.resume(throwing: TransportClientError.cancelled)
                    }
                default:
                    break
                }
            }
            conn.start(queue: .global())
        }
        return conn
    }
}

final class LockedBool: @unchecked Sendable {
    private var value: Bool
    private let lock = NSLock()
    init(_ v: Bool) { value = v }
    func swap(_ new: Bool) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        let old = value
        value = new
        return old
    }
}
