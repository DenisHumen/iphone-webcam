import Foundation

public enum SessionState: Equatable, Sendable {
    case idle
    case connecting
    case handshaking
    case ready(sessionId: String)
    case reconnecting
    case stopped(reason: String)
}
