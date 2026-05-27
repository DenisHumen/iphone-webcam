import ClearCamProtocol
import Foundation

/// Periodically samples `DeviceInfoProvider` and sends `Telemetry` envelopes
/// through `sink`. The caller can `stop()` at any time.
public actor TelemetryEmitter {
    private var task: Task<Void, Never>?
    private let interval: Duration

    public init(interval: Duration = .milliseconds(500)) {
        self.interval = interval
    }

    public func start(
        info: any DeviceInfoProvider,
        sink: @escaping @Sendable (Telemetry) async -> Void
    ) {
        stop()
        let iv = self.interval
        task = Task {
            while !Task.isCancelled {
                let snap = info.snapshot
                let t = Telemetry(
                    tsUsec: UInt64(Date().timeIntervalSince1970 * 1_000_000),
                    batteryLevel: snap.batteryLevel,
                    batteryState: snap.batteryState,
                    thermalState: snap.thermalState,
                    sentBitrateKbps: 0,
                    encFps: 0,
                    captureFps: 0,
                    queueDepth: 0,
                    dropCount: 0
                )
                await sink(t)
                try? await Task.sleep(for: iv)
            }
        }
    }

    public func stop() {
        task?.cancel()
        task = nil
    }
}
