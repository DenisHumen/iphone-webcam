import Foundation

public enum FormatKind: String, Codable, Equatable {
    case raw, encoded
}

public enum CodecName: String, Codable, Equatable {
    case none, hevc, h264
}

public enum PixelFormat: String, Codable, Equatable {
    case nv12
}

public struct Mode: Codable, Equatable {
    public var format: FormatKind
    public var codec: CodecName
    public var width: UInt16
    public var height: UInt16
    public var fps: UInt16
    public var bitrateKbps: UInt32
    public var pixelFormat: PixelFormat
    public var fullRange: Bool

    public init(
        format: FormatKind, codec: CodecName, width: UInt16, height: UInt16, fps: UInt16,
        bitrateKbps: UInt32, pixelFormat: PixelFormat, fullRange: Bool
    ) {
        self.format = format
        self.codec = codec
        self.width = width
        self.height = height
        self.fps = fps
        self.bitrateKbps = bitrateKbps
        self.pixelFormat = pixelFormat
        self.fullRange = fullRange
    }
}

public struct Start: Codable, Equatable {
    public var mode: Mode
    public init(mode: Mode) { self.mode = mode }
}

public struct Stop: Codable, Equatable {
    public init() {}
}

public struct SetMode: Codable, Equatable {
    public var mode: Mode
    public init(mode: Mode) { self.mode = mode }
}

public struct ModeApplied: Codable, Equatable {
    public var mode: Mode
    public var atSeq: UInt32
    public init(mode: Mode, atSeq: UInt32) {
        self.mode = mode
        self.atSeq = atSeq
    }
}
