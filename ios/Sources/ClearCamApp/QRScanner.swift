#if canImport(UIKit) && canImport(AVFoundation)
    import AVFoundation
    import SwiftUI
    import UIKit

    public struct QRScannerView: UIViewControllerRepresentable {
        public let onPayload: (Data) -> Void
        public init(onPayload: @escaping (Data) -> Void) {
            self.onPayload = onPayload
        }

        public func makeUIViewController(context: Context) -> QRScannerVC {
            let vc = QRScannerVC()
            vc.onPayload = onPayload
            return vc
        }

        public func updateUIViewController(_ uiViewController: QRScannerVC, context: Context) {}
    }

    public final class QRScannerVC: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
        var onPayload: ((Data) -> Void)?
        private let session = AVCaptureSession()
        private var preview: AVCaptureVideoPreviewLayer?

        public override func viewDidLoad() {
            super.viewDidLoad()
            view.backgroundColor = .black
            guard let dev = AVCaptureDevice.default(for: .video),
                let input = try? AVCaptureDeviceInput(device: dev),
                session.canAddInput(input)
            else { return }
            session.addInput(input)
            let out = AVCaptureMetadataOutput()
            guard session.canAddOutput(out) else { return }
            session.addOutput(out)
            out.metadataObjectTypes = [.qr]
            out.setMetadataObjectsDelegate(self, queue: .main)
            let layer = AVCaptureVideoPreviewLayer(session: session)
            layer.videoGravity = .resizeAspectFill
            layer.frame = view.bounds
            view.layer.addSublayer(layer)
            preview = layer
            DispatchQueue.global(qos: .userInitiated).async { [weak self] in
                self?.session.startRunning()
            }
        }

        public override func viewDidLayoutSubviews() {
            super.viewDidLayoutSubviews()
            preview?.frame = view.bounds
        }

        public func metadataOutput(
            _ output: AVCaptureMetadataOutput,
            didOutput metadataObjects: [AVMetadataObject],
            from connection: AVCaptureConnection
        ) {
            guard let obj = metadataObjects.first as? AVMetadataMachineReadableCodeObject,
                let s = obj.stringValue,
                let data = s.data(using: .utf8)
            else { return }
            session.stopRunning()
            onPayload?(data)
        }
    }
#endif
