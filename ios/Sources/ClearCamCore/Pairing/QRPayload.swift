import Foundation

public struct QRPayload: Codable, Equatable, Sendable {
    public let v: Int
    public let host: String
    public let cport: Int
    public let mport: Int
    public let token: String

    public init(v: Int, host: String, cport: Int, mport: Int, token: String) {
        self.v = v
        self.host = host
        self.cport = cport
        self.mport = mport
        self.token = token
    }

    public enum DecodeError: Error, Equatable {
        case unsupportedVersion(Int)
        case invalid
    }

    public static let supportedVersion = 1

    public static func decode(_ data: Data) throws -> QRPayload {
        let p = try JSONDecoder().decode(QRPayload.self, from: data)
        guard p.v == supportedVersion else { throw DecodeError.unsupportedVersion(p.v) }
        guard !p.host.isEmpty, p.cport > 0, p.mport > 0, !p.token.isEmpty else {
            throw DecodeError.invalid
        }
        return p
    }

    public static func decode(_ json: String) throws -> QRPayload {
        guard let data = json.data(using: .utf8) else { throw DecodeError.invalid }
        return try decode(data)
    }
}
