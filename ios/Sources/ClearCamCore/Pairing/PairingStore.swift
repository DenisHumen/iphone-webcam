import Foundation
import Security

public enum PairingStoreProvider: Sendable {
    case keychain(service: String)
    case inMemory

    public static func inMemory() -> Self { .inMemory }
}

public actor PairingStore {
    private let provider: PairingStoreProvider
    private var memory: [String: PairingKey] = [:]

    public init(provider: PairingStoreProvider) {
        self.provider = provider
    }

    public func get(udid: String) async throws -> PairingKey? {
        switch provider {
        case .inMemory:
            return memory[udid]
        case .keychain(let service):
            return try keychainRead(service: service, account: udid)
        }
    }

    public func put(udid: String, key: PairingKey) async throws {
        switch provider {
        case .inMemory:
            memory[udid] = key
        case .keychain(let service):
            try keychainWrite(service: service, account: udid, data: key.bytes)
        }
    }

    public func forget(udid: String) async throws {
        switch provider {
        case .inMemory:
            memory.removeValue(forKey: udid)
        case .keychain(let service):
            try keychainDelete(service: service, account: udid)
        }
    }

    // MARK: keychain backend (skipped in CI; covered by manual on-device tests)

    private func keychainWrite(service: String, account: String, data: Data) throws {
        let q: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecValueData as String: data,
        ]
        SecItemDelete(q as CFDictionary)
        let status = SecItemAdd(q as CFDictionary, nil)
        if status != errSecSuccess { throw NSError(domain: "PairingStore", code: Int(status)) }
    }

    private func keychainRead(service: String, account: String) throws -> PairingKey? {
        let q: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var item: CFTypeRef?
        let status = SecItemCopyMatching(q as CFDictionary, &item)
        if status == errSecItemNotFound { return nil }
        if status != errSecSuccess { throw NSError(domain: "PairingStore", code: Int(status)) }
        guard let data = item as? Data else { return nil }
        return PairingKey(data)
    }

    private func keychainDelete(service: String, account: String) throws {
        let q: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
        SecItemDelete(q as CFDictionary)
    }
}
