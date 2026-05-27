import Foundation

public struct Telemetry: Codable, Equatable {
    public var tsUsec: UInt64
    public var batteryLevel: Double
    public var batteryState: BatteryState
    public var thermalState: ThermalState
    public var sentBitrateKbps: UInt32
    public var encFps: UInt32
    public var captureFps: UInt32
    public var queueDepth: UInt32
    public var dropCount: UInt64

    public init(
        tsUsec: UInt64, batteryLevel: Double, batteryState: BatteryState,
        thermalState: ThermalState, sentBitrateKbps: UInt32, encFps: UInt32, captureFps: UInt32,
        queueDepth: UInt32, dropCount: UInt64
    ) {
        self.tsUsec = tsUsec
        self.batteryLevel = batteryLevel
        self.batteryState = batteryState
        self.thermalState = thermalState
        self.sentBitrateKbps = sentBitrateKbps
        self.encFps = encFps
        self.captureFps = captureFps
        self.queueDepth = queueDepth
        self.dropCount = dropCount
    }
}

public struct Ping: Codable, Equatable {
    public var tsUsec: UInt64
    public init(tsUsec: UInt64) { self.tsUsec = tsUsec }
}

public struct Pong: Codable, Equatable {
    public var tsUsec: UInt64
    public var echoUsec: UInt64
    public init(tsUsec: UInt64, echoUsec: UInt64) {
        self.tsUsec = tsUsec
        self.echoUsec = echoUsec
    }
}
