// SwiftPM exposes the @main App from ClearCamAppKit. Xcodegen needs at least
// one source file in the App target for the bundle to assemble; this file
// simply re-exports the App module symbols so the project builds.

import ClearCamAppKit
