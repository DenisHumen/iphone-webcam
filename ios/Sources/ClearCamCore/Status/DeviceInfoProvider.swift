import ClearCamProtocol
import Foundation

public protocol DeviceInfoProvider: Sendable {
    var snapshot: DeviceInfo { get }
}
