import Foundation

public enum BatteryState: String, Codable, Equatable {
    case unknown, unplugged, charging, full
}

public enum ThermalState: String, Codable, Equatable {
    case nominal, fair, serious, critical
}

public enum CameraPosition: String, Codable, Equatable {
    case front, back
}

public struct DeviceInfo: Codable, Equatable {
    public var model: String
    public var osVer: String
    public var batteryLevel: Double
    public var batteryState: BatteryState
    public var thermalState: ThermalState
    public var usb3Capable: Bool

    public init(
        model: String, osVer: String, batteryLevel: Double, batteryState: BatteryState,
        thermalState: ThermalState, usb3Capable: Bool
    ) {
        self.model = model
        self.osVer = osVer
        self.batteryLevel = batteryLevel
        self.batteryState = batteryState
        self.thermalState = thermalState
        self.usb3Capable = usb3Capable
    }
}

public struct CameraEntry: Codable, Equatable {
    public var id: String
    public var name: String
    public var position: CameraPosition
    public var maxWidth: UInt16
    public var maxHeight: UInt16
    public var maxFps: UInt16
    public var supportedFormats: [String]

    public init(
        id: String, name: String, position: CameraPosition, maxWidth: UInt16,
        maxHeight: UInt16, maxFps: UInt16, supportedFormats: [String]
    ) {
        self.id = id
        self.name = name
        self.position = position
        self.maxWidth = maxWidth
        self.maxHeight = maxHeight
        self.maxFps = maxFps
        self.supportedFormats = supportedFormats
    }
}

public struct CameraList: Codable, Equatable {
    public var cameras: [CameraEntry]
    public init(cameras: [CameraEntry]) { self.cameras = cameras }
}

public struct SetCamera: Codable, Equatable {
    public var cameraId: String
    public init(cameraId: String) { self.cameraId = cameraId }
}

public struct CameraState: Codable, Equatable {
    public var activeCameraId: String
    public var appliedFormat: String
    public init(activeCameraId: String, appliedFormat: String) {
        self.activeCameraId = activeCameraId
        self.appliedFormat = appliedFormat
    }
}
