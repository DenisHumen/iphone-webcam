import Foundation

public enum MediaConstants {
    public static let magic: UInt16 = 0xCC01
    public static let headerLen: Int = 28
}

public struct MediaFlags: OptionSet, Hashable {
    public let rawValue: UInt8
    public init(rawValue: UInt8) { self.rawValue = rawValue }
    public static let keyframe = MediaFlags(rawValue: 1 << 0)
    public static let encoded = MediaFlags(rawValue: 1 << 1)
    public static let fullRange = MediaFlags(rawValue: 1 << 2)
    public static let config = MediaFlags(rawValue: 1 << 3)

    /// All defined flag bits (used by decode to mask unknown bits).
    public static let known: MediaFlags = [.keyframe, .encoded, .fullRange, .config]
}

public enum MediaType: UInt8, Hashable {
    case video = 1
}

public enum MediaCodec: UInt8, Hashable {
    case raw = 0
    case hevc = 1
    case h264 = 2
}

public enum MediaHeaderError: Error, Equatable {
    case tooShort(got: Int)
    case badMagic(UInt16)
    case unknownMediaType(UInt8)
    case unknownCodec(UInt8)
}

public struct MediaHeader: Hashable {
    public var mediaType: MediaType
    public var flags: MediaFlags
    public var codec: MediaCodec
    public var width: UInt16
    public var height: UInt16
    public var seq: UInt32
    public var ptsUsec: UInt64
    public var payloadLen: UInt32

    public init(
        mediaType: MediaType,
        flags: MediaFlags,
        codec: MediaCodec,
        width: UInt16,
        height: UInt16,
        seq: UInt32,
        ptsUsec: UInt64,
        payloadLen: UInt32
    ) {
        self.mediaType = mediaType
        self.flags = flags
        self.codec = codec
        self.width = width
        self.height = height
        self.seq = seq
        self.ptsUsec = ptsUsec
        self.payloadLen = payloadLen
    }

    public func encode() -> Data {
        var out = Data(count: MediaConstants.headerLen)
        out.withUnsafeMutableBytes { raw in
            let p = raw.baseAddress!.assumingMemoryBound(to: UInt8.self)
            // magic (big-endian)
            p[0] = UInt8(MediaConstants.magic >> 8)
            p[1] = UInt8(MediaConstants.magic & 0xFF)
            p[2] = mediaType.rawValue
            p[3] = flags.rawValue
            p[4] = codec.rawValue
            // reserved p[5..8] zero (Data(count:) is zeroed)
            p[8] = UInt8(width >> 8); p[9] = UInt8(width & 0xFF)
            p[10] = UInt8(height >> 8); p[11] = UInt8(height & 0xFF)
            writeBE(seq, into: p, offset: 12)
            writeBE64(ptsUsec, into: p, offset: 16)
            writeBE(payloadLen, into: p, offset: 24)
        }
        return out
    }

    public static func decode(from data: Data) throws -> MediaHeader {
        guard data.count >= MediaConstants.headerLen else {
            throw MediaHeaderError.tooShort(got: data.count)
        }
        let i = data.startIndex
        let magic = (UInt16(data[i]) << 8) | UInt16(data[i + 1])
        guard magic == MediaConstants.magic else {
            throw MediaHeaderError.badMagic(magic)
        }
        guard let mediaType = MediaType(rawValue: data[i + 2]) else {
            throw MediaHeaderError.unknownMediaType(data[i + 2])
        }
        let flags = MediaFlags(rawValue: data[i + 3]).intersection(.known)
        guard let codec = MediaCodec(rawValue: data[i + 4]) else {
            throw MediaHeaderError.unknownCodec(data[i + 4])
        }
        // ignore data[5..8] (reserved)
        let width = (UInt16(data[i + 8]) << 8) | UInt16(data[i + 9])
        let height = (UInt16(data[i + 10]) << 8) | UInt16(data[i + 11])
        let seq = readBE32(data, offset: i + 12)
        let pts = readBE64(data, offset: i + 16)
        let plen = readBE32(data, offset: i + 24)
        return MediaHeader(
            mediaType: mediaType, flags: flags, codec: codec,
            width: width, height: height, seq: seq, ptsUsec: pts, payloadLen: plen
        )
    }
}

@inline(__always)
private func writeBE(_ v: UInt32, into p: UnsafeMutablePointer<UInt8>, offset: Int) {
    p[offset] = UInt8((v >> 24) & 0xFF)
    p[offset + 1] = UInt8((v >> 16) & 0xFF)
    p[offset + 2] = UInt8((v >> 8) & 0xFF)
    p[offset + 3] = UInt8(v & 0xFF)
}

@inline(__always)
private func writeBE64(_ v: UInt64, into p: UnsafeMutablePointer<UInt8>, offset: Int) {
    for i in 0..<8 {
        p[offset + i] = UInt8((v >> UInt64(56 - i * 8)) & 0xFF)
    }
}

@inline(__always)
private func readBE32(_ d: Data, offset: Int) -> UInt32 {
    return (UInt32(d[offset]) << 24)
        | (UInt32(d[offset + 1]) << 16)
        | (UInt32(d[offset + 2]) << 8)
        | UInt32(d[offset + 3])
}

@inline(__always)
private func readBE64(_ d: Data, offset: Int) -> UInt64 {
    var v: UInt64 = 0
    for i in 0..<8 {
        v = (v << 8) | UInt64(d[offset + i])
    }
    return v
}
