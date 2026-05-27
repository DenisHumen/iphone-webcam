import Foundation

/// Discriminated union over all CCP control message bodies.
/// Encoded with the parent `ControlEnvelope` into a flat JSON object
/// (`t`/`seq`/`ack` siblings to body fields).
public enum ControlMessage: Equatable {
    case hello(Hello)
    case helloAck(HelloAck)
    case auth(Auth)
    case authOk(AuthOk)
    case mediaHello(MediaHello)
    case bye(Bye)
    case error(ErrorMsg)
    case deviceInfo(DeviceInfo)
    case cameraList(CameraList)
    case setCamera(SetCamera)
    case cameraState(CameraState)
    case start(Start)
    case stop(Stop)
    case setMode(SetMode)
    case modeApplied(ModeApplied)
    case telemetry(Telemetry)
    case ping(Ping)
    case pong(Pong)
    case speedtestStart(SpeedtestStart)
    case speedtestTick(SpeedtestTick)
    case speedtestResult(SpeedtestResult)

    public var typeTag: String {
        switch self {
        case .hello: return "HELLO"
        case .helloAck: return "HELLO_ACK"
        case .auth: return "AUTH"
        case .authOk: return "AUTH_OK"
        case .mediaHello: return "MEDIA_HELLO"
        case .bye: return "BYE"
        case .error: return "ERROR"
        case .deviceInfo: return "DEVICE_INFO"
        case .cameraList: return "CAMERA_LIST"
        case .setCamera: return "SET_CAMERA"
        case .cameraState: return "CAMERA_STATE"
        case .start: return "START"
        case .stop: return "STOP"
        case .setMode: return "SET_MODE"
        case .modeApplied: return "MODE_APPLIED"
        case .telemetry: return "TELEMETRY"
        case .ping: return "PING"
        case .pong: return "PONG"
        case .speedtestStart: return "SPEEDTEST_START"
        case .speedtestTick: return "SPEEDTEST_TICK"
        case .speedtestResult: return "SPEEDTEST_RESULT"
        }
    }
}
