import ClearCamProtocol
import Foundation

/// Source of camera entries. Production uses `AVCameraDiscovery` (iOS only);
/// tests can swap in `StubCameraDiscovery`.
public protocol CameraDiscovery: Sendable {
    func discover() -> [CameraEntry]
}

public struct StubCameraDiscovery: CameraDiscovery {
    public let entries: [CameraEntry]
    public init(_ entries: [CameraEntry]) { self.entries = entries }
    public func discover() -> [CameraEntry] { entries }
}

#if canImport(AVFoundation) && canImport(UIKit)
    import AVFoundation
    import UIKit

    public struct AVCameraDiscovery: CameraDiscovery {
        public init() {}
        public func discover() -> [CameraEntry] {
            let session = AVCaptureDevice.DiscoverySession(
                deviceTypes: [
                    .builtInWideAngleCamera,
                    .builtInUltraWideCamera,
                    .builtInTelephotoCamera,
                    .builtInTrueDepthCamera,
                ],
                mediaType: .video,
                position: .unspecified)
            return session.devices.compactMap { device in
                let position: CameraPosition = device.position == .front ? .front : .back
                let dim = CMVideoFormatDescriptionGetDimensions(device.activeFormat.formatDescription)
                let maxFps = device.formats
                    .flatMap { $0.videoSupportedFrameRateRanges }
                    .map { $0.maxFrameRate }
                    .max() ?? 30
                return CameraEntry(
                    id: device.uniqueID,
                    name: device.localizedName,
                    position: position,
                    maxWidth: UInt16(clamping: Int(dim.width)),
                    maxHeight: UInt16(clamping: Int(dim.height)),
                    maxFps: UInt16(clamping: Int(maxFps)),
                    supportedFormats: ["nv12"]
                )
            }
        }
    }
#endif
