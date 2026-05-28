import Foundation

/// Abstraction over the iPhone capture pipeline. `setCamera()` is the entry
/// the SessionController calls on `SET_CAMERA`. Production impl lives in
/// `AVCaptureEngine` (iOS-only); tests can swap in a recording stub.
public protocol CaptureEngine: AnyObject, Sendable {
    func start(cameraId: String) async throws
    func setCamera(_ cameraId: String) async throws
    func stop() async
}

#if canImport(AVFoundation) && canImport(UIKit)
    import AVFoundation
    import CoreVideo

    public final class AVCaptureEngine: NSObject, CaptureEngine,
        AVCaptureVideoDataOutputSampleBufferDelegate, @unchecked Sendable
    {
        public typealias FrameHandler = @Sendable (CVPixelBuffer, CMTime) -> Void

        private let session = AVCaptureSession()
        private let videoOutput = AVCaptureVideoDataOutput()
        private let queue = DispatchQueue(label: "clearcam.capture", qos: .userInteractive)
        private let onFrame: FrameHandler

        public init(onFrame: @escaping FrameHandler) {
            self.onFrame = onFrame
            super.init()
        }

        public func start(cameraId: String) async throws {
            try configure(cameraId: cameraId, initial: true)
            queue.async { [weak self] in self?.session.startRunning() }
        }

        public func setCamera(_ cameraId: String) async throws {
            try configure(cameraId: cameraId, initial: false)
        }

        public func stop() async {
            queue.async { [weak self] in self?.session.stopRunning() }
        }

        private func configure(cameraId: String, initial: Bool) throws {
            guard let device = AVCaptureDevice(uniqueID: cameraId) else {
                throw NSError(
                    domain: "ClearCam.AVCaptureEngine",
                    code: 1,
                    userInfo: [NSLocalizedDescriptionKey: "camera not found: \(cameraId)"])
            }
            session.beginConfiguration()
            session.inputs.forEach(session.removeInput)
            let input = try AVCaptureDeviceInput(device: device)
            if session.canAddInput(input) { session.addInput(input) }
            if initial {
                videoOutput.videoSettings = [
                    kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_420YpCbCr8BiPlanarFullRange
                ]
                videoOutput.setSampleBufferDelegate(self, queue: queue)
                if session.canAddOutput(videoOutput) {
                    session.addOutput(videoOutput)
                }
            }
            session.commitConfiguration()
        }

        public func captureOutput(
            _ output: AVCaptureOutput,
            didOutput sampleBuffer: CMSampleBuffer,
            from connection: AVCaptureConnection
        ) {
            guard let pb = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
            let pts = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)
            onFrame(pb, pts)
        }
    }
#endif
