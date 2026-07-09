import Foundation
import Network



actor PersistentIPCConnection {
    private var connection: NWConnection?
    private let socketPath: String
    
    init(socketPath: String) {
        self.socketPath = socketPath
    }
    
    private func ensureConnection() async throws -> NWConnection {
        if let conn = connection, conn.state == .ready {
            return conn
        }
        
        // Close old connection if any
        connection?.cancel()
        
        let endpoint = NWEndpoint.unix(path: socketPath)
        let parameters = NWParameters.tcp
        
        let newConnection = NWConnection(to: endpoint, using: parameters)
        
        return try await withCheckedThrowingContinuation { continuation in
            let lock = NSLock()
            var hasResumed = false
            newConnection.stateUpdateHandler = { state in
                let shouldResume: Bool = {
                    lock.lock()
                    defer { lock.unlock() }
                    if hasResumed { return false }
                    hasResumed = true
                    return true
                }()
                
                guard shouldResume else { return }
                
                switch state {
                case .ready:
                    continuation.resume(returning: newConnection)
                case .failed(let error):
                    continuation.resume(throwing: error)
                case .cancelled:
                    continuation.resume(throwing: DeskdropIPCError.disconnected)
                default:
                    lock.lock()
                    hasResumed = false // revert if it was a spurious non-terminal state we didn't handle
                    lock.unlock()
                }
            }
            newConnection.start(queue: .global())
            self.connection = newConnection
        }
    }
    
    func send(cmd: [String: Any]) async throws -> Data {
        let conn = try await ensureConnection()
        let payload = try JSONSerialization.data(withJSONObject: cmd) + Data("\n".utf8)
        
        // Send
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            conn.send(content: payload, completion: .contentProcessed { error in
                if let error = error {
                    continuation.resume(throwing: error)
                } else {
                    continuation.resume()
                }
            })
        }
        
        // Receive line
        return try await receiveLine(from: conn)
    }
    
    private func receiveLine(from conn: NWConnection) async throws -> Data {
        var response = Data()
        while true {
            let chunk = try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Data, Error>) in
                conn.receive(minimumIncompleteLength: 1, maximumLength: 4096) { data, _, isComplete, error in
                    if let error = error {
                        continuation.resume(throwing: error)
                    } else if let data = data {
                        continuation.resume(returning: data)
                    } else if isComplete {
                        continuation.resume(throwing: DeskdropIPCError.disconnected)
                    } else {
                        continuation.resume(throwing: DeskdropIPCError.noData)
                    }
                }
            }
            response.append(chunk)
            if response.last == UInt8(ascii: "\n") {
                break
            }
        }
        return response
    }
}
