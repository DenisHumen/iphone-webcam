#if canImport(UIKit)
    import SwiftUI

    @main
    public struct ClearCamApp: App {
        public init() {}

        public var body: some Scene {
            WindowGroup {
                ConnectView(vm: ConnectViewModel())
                    .preferredColorScheme(.dark)
            }
        }
    }
#endif
