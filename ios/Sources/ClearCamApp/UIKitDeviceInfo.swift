#if canImport(UIKit)
    import ClearCamCore
    import ClearCamProtocol
    import UIKit

    public struct UIKitDeviceInfo: DeviceInfoProvider {
        public init() {
            UIDevice.current.isBatteryMonitoringEnabled = true
        }

        public var snapshot: DeviceInfo {
            let d = UIDevice.current
            let thermal: ThermalState
            switch ProcessInfo.processInfo.thermalState {
            case .nominal: thermal = .nominal
            case .fair: thermal = .fair
            case .serious: thermal = .serious
            case .critical: thermal = .critical
            @unknown default: thermal = .nominal
            }
            let state: BatteryState
            switch d.batteryState {
            case .unknown: state = .unknown
            case .unplugged: state = .unplugged
            case .charging: state = .charging
            case .full: state = .full
            @unknown default: state = .unknown
            }
            return DeviceInfo(
                model: d.model,
                osVer: "\(d.systemName) \(d.systemVersion)",
                batteryLevel: max(0, Double(d.batteryLevel)),
                batteryState: state,
                thermalState: thermal,
                usb3Capable: false  // Phase 5 will detect via lightning vs usb-c.
            )
        }
    }
#endif
