import Foundation

/// Wire-level wrapper carrying `seq`, optional `ack`, and a typed body.
///
/// JSON shape (flattened): `{ "seq": .., "ack": .., "t": "...", ...body fields... }`.
public struct ControlEnvelope: Equatable {
    public var seq: UInt64
    public var ack: UInt64?
    public var body: ControlMessage

    public init(seq: UInt64, ack: UInt64? = nil, body: ControlMessage) {
        self.seq = seq
        self.ack = ack
        self.body = body
    }

    public func encodeJSON() throws -> Data {
        var dict = try ControlEnvelope.bodyDict(self.body)
        dict["t"] = self.body.typeTag
        dict["seq"] = self.seq
        if let ack = self.ack { dict["ack"] = ack }
        return try JSONSerialization.data(withJSONObject: dict, options: [.sortedKeys])
    }

    public static func decodeJSON(_ data: Data) throws -> ControlEnvelope {
        guard let obj = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw ControlEnvelopeError.notAnObject
        }
        guard let t = obj["t"] as? String else { throw ControlEnvelopeError.missingTag }
        let seq = readUInt64(obj["seq"]) ?? 0
        let ack = readUInt64(obj["ack"])

        // Re-encode the body fields only and decode into the typed struct.
        var bodyDict = obj
        bodyDict.removeValue(forKey: "t")
        bodyDict.removeValue(forKey: "seq")
        bodyDict.removeValue(forKey: "ack")
        let bodyData = try JSONSerialization.data(withJSONObject: bodyDict, options: [.sortedKeys])
        let dec = JSONDecoder()

        let body: ControlMessage
        switch t {
        case "HELLO": body = .hello(try dec.decode(Hello.self, from: bodyData))
        case "HELLO_ACK": body = .helloAck(try dec.decode(HelloAck.self, from: bodyData))
        case "AUTH": body = .auth(try dec.decode(Auth.self, from: bodyData))
        case "AUTH_OK": body = .authOk(try dec.decode(AuthOk.self, from: bodyData))
        case "MEDIA_HELLO": body = .mediaHello(try dec.decode(MediaHello.self, from: bodyData))
        case "BYE": body = .bye(try dec.decode(Bye.self, from: bodyData))
        case "ERROR": body = .error(try dec.decode(ErrorMsg.self, from: bodyData))
        case "DEVICE_INFO": body = .deviceInfo(try dec.decode(DeviceInfo.self, from: bodyData))
        case "CAMERA_LIST": body = .cameraList(try dec.decode(CameraList.self, from: bodyData))
        case "SET_CAMERA": body = .setCamera(try dec.decode(SetCamera.self, from: bodyData))
        case "CAMERA_STATE": body = .cameraState(try dec.decode(CameraState.self, from: bodyData))
        case "START": body = .start(try dec.decode(Start.self, from: bodyData))
        case "STOP": body = .stop(try dec.decode(Stop.self, from: bodyData))
        case "SET_MODE": body = .setMode(try dec.decode(SetMode.self, from: bodyData))
        case "MODE_APPLIED": body = .modeApplied(try dec.decode(ModeApplied.self, from: bodyData))
        case "TELEMETRY": body = .telemetry(try dec.decode(Telemetry.self, from: bodyData))
        case "PING": body = .ping(try dec.decode(Ping.self, from: bodyData))
        case "PONG": body = .pong(try dec.decode(Pong.self, from: bodyData))
        case "SPEEDTEST_START":
            body = .speedtestStart(try dec.decode(SpeedtestStart.self, from: bodyData))
        case "SPEEDTEST_TICK":
            body = .speedtestTick(try dec.decode(SpeedtestTick.self, from: bodyData))
        case "SPEEDTEST_RESULT":
            body = .speedtestResult(try dec.decode(SpeedtestResult.self, from: bodyData))
        default: throw ControlEnvelopeError.unknownType(t)
        }
        return ControlEnvelope(seq: seq, ack: ack, body: body)
    }

    /// Encode the body fields (NOT including `t`/`seq`/`ack`) to a [String: Any] dict.
    private static func bodyDict(_ body: ControlMessage) throws -> [String: Any] {
        let enc = JSONEncoder()
        let data: Data
        switch body {
        case .hello(let v): data = try enc.encode(v)
        case .helloAck(let v): data = try enc.encode(v)
        case .auth(let v): data = try enc.encode(v)
        case .authOk(let v): data = try enc.encode(v)
        case .mediaHello(let v): data = try enc.encode(v)
        case .bye(let v): data = try enc.encode(v)
        case .error(let v): data = try enc.encode(v)
        case .deviceInfo(let v): data = try enc.encode(v)
        case .cameraList(let v): data = try enc.encode(v)
        case .setCamera(let v): data = try enc.encode(v)
        case .cameraState(let v): data = try enc.encode(v)
        case .start(let v): data = try enc.encode(v)
        case .stop(let v): data = try enc.encode(v)
        case .setMode(let v): data = try enc.encode(v)
        case .modeApplied(let v): data = try enc.encode(v)
        case .telemetry(let v): data = try enc.encode(v)
        case .ping(let v): data = try enc.encode(v)
        case .pong(let v): data = try enc.encode(v)
        case .speedtestStart(let v): data = try enc.encode(v)
        case .speedtestTick(let v): data = try enc.encode(v)
        case .speedtestResult(let v): data = try enc.encode(v)
        }
        guard let dict = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw ControlEnvelopeError.notAnObject
        }
        return dict
    }
}

public enum ControlEnvelopeError: Error, Equatable {
    case notAnObject
    case missingTag
    case unknownType(String)
}

/// Length-prefixed framing for the control channel.
public enum ControlFraming {
    public static let prefixLen: Int = 4
    public static let defaultMaxPayload: UInt32 = 1 << 20  // 1 MiB

    public enum FrameError: Error, Equatable {
        case tooLarge(len: UInt32, max: UInt32)
    }

    public static func frame(_ payload: Data) -> Data {
        var out = Data(capacity: prefixLen + payload.count)
        let len = UInt32(payload.count)
        out.append(UInt8((len >> 24) & 0xFF))
        out.append(UInt8((len >> 16) & 0xFF))
        out.append(UInt8((len >> 8) & 0xFF))
        out.append(UInt8(len & 0xFF))
        out.append(payload)
        return out
    }

    /// Returns `(payload, total)` if a full frame is present, `nil` if more bytes are needed.
    public static func tryUnframe(_ buf: Data, maxPayload: UInt32 = defaultMaxPayload) throws
        -> (Data, Int)?
    {
        guard buf.count >= prefixLen else { return nil }
        let i = buf.startIndex
        let len =
            (UInt32(buf[i]) << 24)
            | (UInt32(buf[i + 1]) << 16)
            | (UInt32(buf[i + 2]) << 8)
            | UInt32(buf[i + 3])
        if len > maxPayload { throw FrameError.tooLarge(len: len, max: maxPayload) }
        let total = prefixLen + Int(len)
        guard buf.count >= total else { return nil }
        let payload = buf.subdata(in: (i + prefixLen)..<(i + total))
        return (payload, total)
    }
}

/// Accepts both UInt64 and Int values for compatibility with JSONSerialization's NSNumber bridging.
private func readUInt64(_ any: Any?) -> UInt64? {
    if let v = any as? UInt64 { return v }
    if let v = any as? Int64 { return UInt64(v) }
    if let v = any as? Int { return UInt64(v) }
    if let v = any as? NSNumber { return v.uint64Value }
    return nil
}
