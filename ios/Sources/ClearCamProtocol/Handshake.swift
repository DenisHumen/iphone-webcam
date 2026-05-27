import Foundation

public struct DeviceIdent: Codable, Equatable {
    public var model: String
    public var osVer: String

    public init(model: String, osVer: String) {
        self.model = model
        self.osVer = osVer
    }
}

public struct Hello: Codable, Equatable {
    public var protoVer: UInt32
    public var app: String
    public var device: DeviceIdent
    public var sessionId: String
    public var caps: [Capability]

    public init(
        protoVer: UInt32, app: String, device: DeviceIdent, sessionId: String,
        caps: [Capability]
    ) {
        self.protoVer = protoVer
        self.app = app
        self.device = device
        self.sessionId = sessionId
        self.caps = caps
    }
}

public struct HelloAck: Codable, Equatable {
    public var protoVer: UInt32
    public var caps: [Capability]
    public init(protoVer: UInt32, caps: [Capability]) {
        self.protoVer = protoVer
        self.caps = caps
    }
}

public struct Auth: Codable, Equatable {
    public var token: String
    public init(token: String) { self.token = token }
}

public struct AuthOk: Codable, Equatable {
    public var sessionId: String
    public init(sessionId: String) { self.sessionId = sessionId }
}

public struct MediaHello: Codable, Equatable {
    public var sessionId: String
    public var token: String
    public init(sessionId: String, token: String) {
        self.sessionId = sessionId
        self.token = token
    }
}

public struct Bye: Codable, Equatable {
    public var reason: String
    public init(reason: String) { self.reason = reason }
}

public enum ErrorCode: String, Codable, Equatable {
    case incompatibleVersion = "incompatible_version"
    case unauthorized
    case badRequest = "bad_request"
    case unsupportedMode = "unsupported_mode"
    case cameraUnavailable = "camera_unavailable"
    case `internal`
    case timeout
}

/// Error body. `seq` of the failing request lives on the envelope's `ack`, NOT here.
public struct ErrorMsg: Codable, Equatable {
    public var code: ErrorCode
    public var message: String
    public init(code: ErrorCode, message: String) {
        self.code = code
        self.message = message
    }
}
