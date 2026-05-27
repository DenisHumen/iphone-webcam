import Foundation

public enum ClearCamProtocolVersion {
    /// Current CCP protocol version. Mirrors `ccp_protocol::PROTO_VER` in Rust.
    public static let current: UInt32 = 1
}

public enum Capability: Hashable {
    case hevc
    case h264
    case rawNv12
    case usb3
    case speedtestV1
    case other(String)

    public var wireString: String {
        switch self {
        case .hevc: return "hevc"
        case .h264: return "h264"
        case .rawNv12: return "raw_nv12"
        case .usb3: return "usb3"
        case .speedtestV1: return "speedtest_v1"
        case .other(let s): return s
        }
    }

    public init(wireString s: String) {
        switch s {
        case "hevc": self = .hevc
        case "h264": self = .h264
        case "raw_nv12": self = .rawNv12
        case "usb3": self = .usb3
        case "speedtest_v1": self = .speedtestV1
        default: self = .other(s)
        }
    }
}

extension Capability: Codable {
    public init(from decoder: Decoder) throws {
        let s = try decoder.singleValueContainer().decode(String.self)
        self = Capability(wireString: s)
    }
    public func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        try c.encode(wireString)
    }
}
