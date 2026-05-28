import ClearCamProtocol
import Foundation

public struct EncoderMode: Equatable, Sendable {
    public let codec: MediaCodec
    public let width: UInt16
    public let height: UInt16
    public let fps: UInt16
    public let bitrateKbps: UInt32

    public init(codec: MediaCodec, width: UInt16, height: UInt16, fps: UInt16, bitrateKbps: UInt32) {
        self.codec = codec
        self.width = width
        self.height = height
        self.fps = fps
        self.bitrateKbps = bitrateKbps
    }
}

public struct EncodedAU: Equatable, Sendable {
    public let isKeyframe: Bool
    public let isConfig: Bool
    /// Each entry is a single NAL unit body (no length prefix).
    public let nalUnits: [Data]
    public let ptsUsec: UInt64

    public init(isKeyframe: Bool, isConfig: Bool, nalUnits: [Data], ptsUsec: UInt64) {
        self.isKeyframe = isKeyframe
        self.isConfig = isConfig
        self.nalUnits = nalUnits
        self.ptsUsec = ptsUsec
    }

    /// Pack the AU into the on-the-wire body: concatenated `[u32 BE len][nal bytes]`.
    public func wireBody() -> Data {
        var out = Data()
        for nal in nalUnits {
            let len = UInt32(nal.count)
            var lenBytes = [
                UInt8((len >> 24) & 0xFF),
                UInt8((len >> 16) & 0xFF),
                UInt8((len >> 8) & 0xFF),
                UInt8(len & 0xFF),
            ]
            out.append(contentsOf: lenBytes)
            out.append(nal)
            lenBytes.removeAll()
        }
        return out
    }
}

/// Abstraction over the iPhone-side video encoder. Production impl is
/// `VTEncoder` (VideoToolbox); tests use `StubEncoder` to assert framing.
public protocol Encoder: Sendable {
    func configure(mode: EncoderMode) async throws
    /// Caller hands in a CVPixelBuffer (or, in tests, a synthetic stand-in).
    /// The encoder emits zero or one access unit via `onAccessUnit`.
    func encode(
        _ frameId: UInt32,
        ptsUsec: UInt64,
        onAccessUnit: @Sendable (EncodedAU) -> Void
    ) async throws
    func stop() async
}

/// Test stub: produces a 1-NAL "fake" AU per call; every 30 frames marks as
/// keyframe; the very first call emits an additional config AU (VPS/SPS/PPS).
public actor StubEncoder: Encoder {
    public var mode: EncoderMode
    private var calls: UInt32 = 0
    private var configEmitted = false

    public init(mode: EncoderMode) {
        self.mode = mode
    }

    public func configure(mode: EncoderMode) async throws {
        self.mode = mode
        configEmitted = false
    }

    public func encode(
        _ frameId: UInt32,
        ptsUsec: UInt64,
        onAccessUnit: @Sendable (EncodedAU) -> Void
    ) async throws {
        if !configEmitted {
            configEmitted = true
            let vps = Data([0x40, 0x01])
            let sps = Data([0x42, 0x01])
            let pps = Data([0x44, 0x01])
            onAccessUnit(
                EncodedAU(
                    isKeyframe: false,
                    isConfig: true,
                    nalUnits: [vps, sps, pps],
                    ptsUsec: ptsUsec))
        }
        calls &+= 1
        let isKey = calls % 30 == 1
        let payload = Data(repeating: UInt8(frameId & 0xFF), count: 32)
        onAccessUnit(
            EncodedAU(
                isKeyframe: isKey,
                isConfig: false,
                nalUnits: [payload],
                ptsUsec: ptsUsec))
    }

    public func stop() async {}
}
