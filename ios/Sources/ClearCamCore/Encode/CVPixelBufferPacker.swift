#if canImport(CoreVideo)
    import ClearCamProtocol
    import CoreVideo
    import Foundation

    public enum CVPixelBufferPackerError: Error, Equatable {
        case lockFailed
        case planeMissing
    }

    public enum CVPixelBufferPacker {
        /// Lock a CVPixelBuffer (`kCVPixelFormatType_420YpCbCr8BiPlanarFullRange`),
        /// copy planes into tight buffers, and return a `PackedFrame`.
        public static func packNV12(
            _ pixelBuffer: CVPixelBuffer,
            seq: UInt32,
            ptsUsec: UInt64
        ) throws -> PackedFrame {
            guard CVPixelBufferLockBaseAddress(pixelBuffer, .readOnly) == kCVReturnSuccess else {
                throw CVPixelBufferPackerError.lockFailed
            }
            defer { CVPixelBufferUnlockBaseAddress(pixelBuffer, .readOnly) }
            let width = CVPixelBufferGetWidth(pixelBuffer)
            let height = CVPixelBufferGetHeight(pixelBuffer)
            guard CVPixelBufferGetPlaneCount(pixelBuffer) == 2,
                let yBase = CVPixelBufferGetBaseAddressOfPlane(pixelBuffer, 0),
                let uvBase = CVPixelBufferGetBaseAddressOfPlane(pixelBuffer, 1)
            else {
                throw CVPixelBufferPackerError.planeMissing
            }
            let yStride = CVPixelBufferGetBytesPerRowOfPlane(pixelBuffer, 0)
            let uvStride = CVPixelBufferGetBytesPerRowOfPlane(pixelBuffer, 1)

            var y = Data(count: width * height)
            var uv = Data(count: width * height / 2)
            y.withUnsafeMutableBytes { dst in
                guard let base = dst.baseAddress else { return }
                for r in 0..<height {
                    memcpy(base.advanced(by: r * width), yBase.advanced(by: r * yStride), width)
                }
            }
            uv.withUnsafeMutableBytes { dst in
                guard let base = dst.baseAddress else { return }
                for r in 0..<(height / 2) {
                    memcpy(base.advanced(by: r * width), uvBase.advanced(by: r * uvStride), width)
                }
            }
            return try RawEncoder.packNV12(
                y: y,
                uv: uv,
                width: UInt16(width),
                height: UInt16(height),
                seq: seq,
                ptsUsec: ptsUsec,
                fullRange: true
            )
        }
    }
#endif
