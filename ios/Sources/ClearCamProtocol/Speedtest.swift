import Foundation

public enum SpeedtestPattern: String, Codable, Equatable {
    case ramp
}

public struct SpeedtestStart: Codable, Equatable {
    public var id: String
    public var targetBitrateKbps: UInt32
    public var durationMs: UInt32
    public var pattern: SpeedtestPattern

    public init(
        id: String, targetBitrateKbps: UInt32, durationMs: UInt32, pattern: SpeedtestPattern
    ) {
        self.id = id
        self.targetBitrateKbps = targetBitrateKbps
        self.durationMs = durationMs
        self.pattern = pattern
    }
}

/// Inner counter is `tickIndex` (not `seq`) so it does not collide with the
/// envelope's `seq` once flattened. `id` matches [`SpeedtestStart.id`].
public struct SpeedtestTick: Codable, Equatable {
    public var id: String
    public var tickIndex: UInt32
    public var tsUsec: UInt64
    public init(id: String, tickIndex: UInt32, tsUsec: UInt64) {
        self.id = id
        self.tickIndex = tickIndex
        self.tsUsec = tsUsec
    }
}

public struct SpeedtestResult: Codable, Equatable {
    public var id: String
    public var goodputMbps: Double
    public var rttMs: Double
    public var jitterMs: Double
    public var lossPct: Double
    public var recommendedMode: Mode

    public init(
        id: String, goodputMbps: Double, rttMs: Double, jitterMs: Double, lossPct: Double,
        recommendedMode: Mode
    ) {
        self.id = id
        self.goodputMbps = goodputMbps
        self.rttMs = rttMs
        self.jitterMs = jitterMs
        self.lossPct = lossPct
        self.recommendedMode = recommendedMode
    }
}
