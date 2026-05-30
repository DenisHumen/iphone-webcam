import Foundation
import Network

public enum TransportListenerError: Error, Sendable {
    case invalidPort
    case startFailed(String)
}

public actor TransportListener {
    public struct BoundPorts: Sendable {
        public let control: NWEndpoint.Port
        public let media: NWEndpoint.Port
    }

    public struct InboundPair: Sendable {
        public let control: ControlStream
        public let media: ControlStream
    }

    private let controlPort: NWEndpoint.Port
    private let mediaPort: NWEndpoint.Port
    private var controlListener: NWListener?
    private var mediaListener: NWListener?
    // Pending streams from real NWConnection arrivals.
    private var pendingControl: [ControlStream] = []
    private var pendingMedia: [ControlStream] = []
    private var sessionContinuations: [CheckedContinuation<InboundPair, Error>] = []

    public init(controlPort: Int, mediaPort: Int) throws {
        // controlPort == 0 / mediaPort == 0 means "any free port"; NWEndpoint.Port handles 0 too.
        guard let c = NWEndpoint.Port(rawValue: UInt16(controlPort)),
              let m = NWEndpoint.Port(rawValue: UInt16(mediaPort))
        else { throw TransportListenerError.invalidPort }
        self.controlPort = c
        self.mediaPort = m
    }

    public func start() async throws -> BoundPorts {
        let cParams = NWParameters.tcp
        cParams.allowLocalEndpointReuse = true
        let mParams = NWParameters.tcp
        mParams.allowLocalEndpointReuse = true
        let cListener: NWListener
        let mListener: NWListener
        do {
            cListener = try NWListener(using: cParams, on: controlPort)
            mListener = try NWListener(using: mParams, on: mediaPort)
        } catch {
            throw TransportListenerError.startFailed(String(describing: error))
        }

        cListener.newConnectionHandler = { [weak self] conn in
            conn.start(queue: .global())
            Task { await self?.acceptControl(conn) }
        }
        mListener.newConnectionHandler = { [weak self] conn in
            conn.start(queue: .global())
            Task { await self?.acceptMedia(conn) }
        }

        cListener.start(queue: .global())
        mListener.start(queue: .global())
        self.controlListener = cListener
        self.mediaListener = mListener

        // Poll briefly for both listeners to publish their assigned ports
        // (matters when `controlPort == 0` selects an ephemeral port).
        for _ in 0..<50 {
            if let cp = cListener.port, let mp = mListener.port {
                return BoundPorts(control: cp, media: mp)
            }
            try await Task.sleep(nanoseconds: 20_000_000)
        }
        throw TransportListenerError.startFailed("listener never published a port")
    }

    public func nextSession() async throws -> InboundPair {
        try await withCheckedThrowingContinuation { (cont: CheckedContinuation<InboundPair, Error>) in
            Task { await self.appendContinuation(cont) }
        }
    }

    public func stop() {
        controlListener?.cancel()
        mediaListener?.cancel()
        controlListener = nil
        mediaListener = nil
    }

    // MARK: internal test hook

    /// Directly enqueue a pre-built pair of streams. Used by unit tests that
    /// verify the pairing logic without starting a real NWListener.
    func _inject(control: ControlStream, media: ControlStream) {
        pendingControl.append(control)
        pendingMedia.append(media)
        tryMatch()
    }

    // MARK: private helpers

    private func appendContinuation(_ cont: CheckedContinuation<InboundPair, Error>) {
        sessionContinuations.append(cont)
        tryMatch()
    }

    private func acceptControl(_ conn: NWConnection) {
        pendingControl.append(ControlStream(io: NWConnectionIO(connection: conn)))
        tryMatch()
    }

    private func acceptMedia(_ conn: NWConnection) {
        pendingMedia.append(ControlStream(io: NWConnectionIO(connection: conn)))
        tryMatch()
    }

    private func tryMatch() {
        while !sessionContinuations.isEmpty, !pendingControl.isEmpty, !pendingMedia.isEmpty {
            let cont = sessionContinuations.removeFirst()
            let cStream = pendingControl.removeFirst()
            let mStream = pendingMedia.removeFirst()
            let pair = InboundPair(control: cStream, media: mStream)
            cont.resume(returning: pair)
        }
    }
}
