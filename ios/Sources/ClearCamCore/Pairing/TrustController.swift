import Foundation
import Combine

public enum TrustOutcome: Sendable, Equatable {
    case accepted(PairingKey)
    case denied
}

public struct TrustPrompt: Sendable, Equatable {
    public let desktopId: String
}

@MainActor
public final class TrustController: ObservableObject {
    @Published public private(set) var pendingPrompt: TrustPrompt?
    private var pendingContinuation: CheckedContinuation<TrustOutcome, Never>?
    private let store: PairingStore

    public init(store: PairingStore) {
        self.store = store
    }

    public func requestTrust(forDesktopId id: String) async -> TrustOutcome {
        await withCheckedContinuation { (cont: CheckedContinuation<TrustOutcome, Never>) in
            pendingPrompt = TrustPrompt(desktopId: id)
            pendingContinuation = cont
        }
    }

    public func respond(accept: Bool) {
        guard let prompt = pendingPrompt, let cont = pendingContinuation else { return }
        pendingPrompt = nil
        pendingContinuation = nil
        if accept {
            let key = PairingKey.random()
            // Fire-and-forget persistence to the actor; the test polls.
            Task {
                try? await store.put(udid: prompt.desktopId, key: key)
            }
            cont.resume(returning: .accepted(key))
        } else {
            cont.resume(returning: .denied)
        }
    }
}
