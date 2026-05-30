import Foundation
import Security

public struct PairingKey: Sendable, Equatable {
    public let bytes: Data
    public init(_ bytes: Data) { self.bytes = bytes }
    public static func random() -> PairingKey {
        var b = Data(count: 32)
        _ = b.withUnsafeMutableBytes { SecRandomCopyBytes(kSecRandomDefault, 32, $0.baseAddress!) }
        return PairingKey(b)
    }
    public var base64NoPad: String {
        bytes.base64EncodedString().trimmingCharacters(in: CharacterSet(charactersIn: "="))
    }
}
