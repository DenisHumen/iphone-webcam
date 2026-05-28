import ClearCamProtocol
import Foundation

public enum RawEncoderError: Error, Equatable {
    case planeSizeMismatch(gotY: Int, gotUV: Int, expectedY: Int, expectedUV: Int)
}

/// A packed media frame ready to write on the wire: 28-byte header + payload.
public struct PackedFrame: Sendable, Equatable {
    public let header: MediaHeader
    /// `Y plane bytes` followed by `interleaved CbCr plane bytes`.
    public let payload: Data

    public init(header: MediaHeader, payload: Data) {
        self.header = header
        self.payload = payload
    }

    /// Header bytes + payload bytes as one `Data`, ready for `MediaStream` send.
    public func wireBytes() -> Data {
        var out = header.encode()
        out.append(payload)
        return out
    }
}

public enum RawEncoder {
    /// Pack a pair of NV12 planes into a `PackedFrame` for transmission.
    ///
    /// Tight packing is enforced (stride == width). When the caller has
    /// rows wider than `width`, copy into a tight buffer first.
    public static func packNV12(
        y: Data,
        uv: Data,
        width: UInt16,
        height: UInt16,
        seq: UInt32,
        ptsUsec: UInt64,
        fullRange: Bool = true
    ) throws -> PackedFrame {
        let expectedY = Int(width) * Int(height)
        let expectedUV = Int(width) * Int(height) / 2
        guard y.count == expectedY, uv.count == expectedUV else {
            throw RawEncoderError.planeSizeMismatch(
                gotY: y.count, gotUV: uv.count, expectedY: expectedY, expectedUV: expectedUV)
        }
        var payload = Data(capacity: expectedY + expectedUV)
        payload.append(y)
        payload.append(uv)
        let header = MediaHeader(
            mediaType: .video,
            flags: fullRange ? [.fullRange] : [],
            codec: .raw,
            width: width,
            height: height,
            seq: seq,
            ptsUsec: ptsUsec,
            payloadLen: UInt32(payload.count)
        )
        return PackedFrame(header: header, payload: payload)
    }
}
