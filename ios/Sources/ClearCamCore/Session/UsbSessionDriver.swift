import Foundation

/// Resolves the pairing credential for an inbound USB connection and drives the
/// `SessionController` handshake.
///
/// For every connection arriving on a USB-muxd socket the caller creates one
/// `UsbSessionDriver`, calls `start()`, and awaits the ready `SessionController`.
///
/// Credential resolution order:
/// 1. If `PairingStore` already holds a key for `peerId` — use it immediately.
/// 2. Otherwise — surface a trust prompt via `TrustController`.
///    - `.accepted(key)` → persist (TrustController handles that), then handshake.
///    - `.denied`        → send BYE and throw `SessionError.trustDenied`.
public actor UsbSessionDriver {
    private let pair: TransportListener.InboundPair
    private let store: PairingStore
    private let trust: TrustController
    private let peerId: String
    private let deviceInfo: any DeviceInfoProvider

    public init(
        pair: TransportListener.InboundPair,
        store: PairingStore,
        trust: TrustController,
        peerId: String,
        deviceInfo: any DeviceInfoProvider
    ) {
        self.pair = pair
        self.store = store
        self.trust = trust
        self.peerId = peerId
        self.deviceInfo = deviceInfo
    }

    /// Resolve credential, then run the handshake.  Returns the ready `SessionController`.
    @discardableResult
    public func start() async throws -> SessionController {
        let credential = try await resolveCredential()
        let controller = SessionController(
            controlChannel: pair.control,
            mediaChannel: pair.media,
            deviceInfo: deviceInfo
        )
        try await controller.connect(credential: credential)
        return controller
    }

    // MARK: Private

    private func resolveCredential() async throws -> AuthCredential {
        if let stored = try await store.get(udid: peerId) {
            return .pairingKey(stored)
        }
        let outcome = await trust.requestTrust(forDesktopId: peerId)
        switch outcome {
        case .accepted(let key):
            return .pairingKey(key)
        case .denied:
            // Close the transport before propagating.
            await pair.control.cancel()
            await pair.media.cancel()
            throw SessionError.trustDenied
        }
    }
}
