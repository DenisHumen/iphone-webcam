public enum TransportRole: Sendable {
    case client(host: String, cport: Int, mport: Int)
    case listener(cport: Int, mport: Int)
}
