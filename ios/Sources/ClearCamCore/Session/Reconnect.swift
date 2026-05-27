import Foundation

/// Exponential backoff with jitter for reconnect attempts.
public struct Reconnect: Sendable {
    public let base: Duration
    public let cap: Duration
    public let factor: Double

    public init(
        base: Duration = .milliseconds(500),
        cap: Duration = .seconds(15),
        factor: Double = 2.0
    ) {
        self.base = base
        self.cap = cap
        self.factor = factor
    }

    public func delay(forAttempt attempt: Int) -> Duration {
        precondition(attempt >= 0)
        let exp = pow(factor, Double(attempt))
        let baseMillis = Double(base.components.seconds) * 1000
            + Double(base.components.attoseconds) / 1e15
        let capMillis = Double(cap.components.seconds) * 1000
            + Double(cap.components.attoseconds) / 1e15
        let raw = min(baseMillis * exp, capMillis)
        let jittered = raw * Double.random(in: 0.85...1.15)
        return .milliseconds(Int(jittered))
    }
}
