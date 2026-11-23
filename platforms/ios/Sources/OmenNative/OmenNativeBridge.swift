import Foundation
import UIKit
import AVFoundation
import UserNotifications
import COmenBridge
import Darwin

private final class WeakBridge { weak var value: OmenNativeBridge?; init(_ value: OmenNativeBridge) { self.value = value } }
private func omenSwiftSend(_ context: UInt64, _ pointer: UnsafePointer<UInt8>?, _ count: Int) -> Bool {
    guard let pointer, count > 0, count <= 1_048_576 else { return false }
    let bytes = Data(bytes: pointer, count: count) // Copy before the C callback returns.
    Task { @MainActor in OmenNativeBridge.live[context]?.value?.dispatch(bytes) }
    return true
}
private enum NativeFailure: Error { case denied, unsupported, invalid, failed, busy }

/// Own from the application scene and close explicitly before destroying the Rust runtime.
@MainActor public final class OmenNativeBridge: NSObject, UIImagePickerControllerDelegate, UINavigationControllerDelegate {
    fileprivate static var live: [UInt64: WeakBridge] = [:]
    private static var next: UInt64 = 1
    public private(set) var handle: UInt64 = 0
    private let token: UInt64
    private let root: URL
    private let allowlist: Set<String>
    private let responseSink: ((Data)->Void)?
    private weak var presenter: UIViewController?
    private var calls: [String: Task<Void, Never>] = [:]
    private var closed = false
    private var photoWaiter: CheckedContinuation<UIImage, Error>?
    private var picker: UIImagePickerController?

    public init(presenter: UIViewController, allowlist: Set<String>, root: URL, responseSink: ((Data)->Void)? = nil) throws {
        guard Self.next < UInt64.max else { throw NativeFailure.failed }
        token = Self.next; Self.next += 1
        self.presenter = presenter; self.allowlist = allowlist; self.root = root;self.responseSink=responseSink
        super.init()
        Self.live[token] = WeakBridge(self)
        handle = omen_bridge_create(omenSwiftSend, token)
        guard handle != 0 else { Self.live.removeValue(forKey: token); throw NativeFailure.failed }
    }
    deinit { omen_bridge_destroy(handle) }
    public func close() {
        guard !closed else { return }; closed = true
        calls.values.forEach { $0.cancel() }; calls.removeAll()
        photoWaiter?.resume(throwing: CancellationError()); photoWaiter = nil
        picker?.dismiss(animated: false); picker = nil
        Self.live.removeValue(forKey: token); omen_bridge_destroy(handle)
    }
    /// Call from a user action and synchronize the returned grant into the Rust host broker.
    public func requestPermission(_ permission: String) async -> String {
        guard allowlist.contains(permission), !closed else { return "denied" }
        switch permission {
        case "fs.read", "fs.write": return "granted"
        case "camera": return await AVCaptureDevice.requestAccess(for: .video) ? "granted" : "denied"
        case "notifications":
            return (try? await UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound, .badge])) == true ? "granted" : "denied"
        default: return "denied"
        }
    }
    private func requirePermission(_ permission: String) async throws {
        guard allowlist.contains(permission), !closed else { throw NativeFailure.denied }
        switch permission {
        case "fs.read", "fs.write": break
        case "camera": guard AVCaptureDevice.authorizationStatus(for: .video) == .authorized else { throw NativeFailure.denied }
        case "notifications":
            let status = await UNUserNotificationCenter.current().notificationSettings().authorizationStatus
            guard status == .authorized || status == .provisional || status == .ephemeral else { throw NativeFailure.denied }
        default: throw NativeFailure.denied
        }
    }
    /// Dispatch a reviewed host request received from the bundled WebView application.
    public func dispatchHost(_ bytes:Data){guard bytes.count<=1_048_576 else{return};dispatch(bytes)}
    /// Cancel in-flight operations when a WebView process is lost without disposing this host.
    public func cancelPending(){calls.values.forEach{$0.cancel()};calls.removeAll();cancelPhoto()}
    fileprivate func dispatch(_ bytes: Data) {
        guard !closed, let message = try? JSONSerialization.jsonObject(with: bytes) as? [String: Any],
              message["api_version"] as? Int == 1, let id = message["id"] as? String,
              !id.isEmpty, id.count <= 20, id.allSatisfy({ $0.isASCII && $0.isNumber }) else { return }
        if message["type"] as? String == "cancel" { calls.removeValue(forKey: id)?.cancel(); return }
        guard message["type"] as? String == "call", calls[id] == nil else { return }
        guard calls.count < 256 else { failure(id, code: "unavailable"); return }
        calls[id] = Task { [weak self] in
            guard let self else { return }
            defer { calls.removeValue(forKey: id) }
            do {
                guard let plugin = message["plugin"] as? String, let operation = message["operation"] as? String, let payload = message["payload"] as? [String: Any] else { throw NativeFailure.invalid }
                let output = try await invoke(plugin: plugin, operation: operation, payload: payload)
                try Task.checkCancellation()
                send(["type": "response", "api_version": 1, "id": id, "payload": output])
            } catch is CancellationError { if !Task.isCancelled { failure(id, code: "cancelled") } }
              catch NativeFailure.denied { failure(id, code: "permission_denied") }
              catch NativeFailure.unsupported { failure(id, code: "unsupported") }
              catch NativeFailure.invalid { failure(id, code: "invalid_request") }
              catch { failure(id, code: "native") }
        }
    }
    private func invoke(plugin: String, operation: String, payload: [String: Any]) async throws -> Any {
        switch "\(plugin)/\(operation)" {
        case "omen.filesystem/read_text", "omen.filesystem/write_text":
            let writing = operation == "write_text"
            try await requirePermission(writing ? "fs.write" : "fs.read")
            guard let path = payload["path"] as? String else { throw NativeFailure.invalid }
            let contents = writing ? payload["contents"] as? String : nil
            if writing && contents == nil { throw NativeFailure.invalid }
            let root = self.root
            let output = try await Task.detached { try Self.fileOperation(root: root, path: path, contents: contents) }.value
            try Task.checkCancellation()
            if writing { send(["type": "event", "api_version": 1, "plugin": plugin, "topic": "changed", "payload": ["path": path]]) }
            return writing ? NSNull() : output
        case "omen.notifications/show":
            try await requirePermission("notifications")
            guard let title = payload["title"] as? String, let body = payload["body"] as? String, title.utf8.count <= 256, body.utf8.count <= 4096 else { throw NativeFailure.invalid }
            let content = UNMutableNotificationContent(); content.title = title; content.body = body
            try Task.checkCancellation()
            try await UNUserNotificationCenter.current().add(UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: nil))
            return NSNull()
        case "omen.camera/capture":
            try await requirePermission("camera")
            guard UIImagePickerController.isSourceTypeAvailable(.camera), let presenter, picker == nil else { throw NativeFailure.unsupported }
            guard let width = payload["width"] as? Int, let height = payload["height"] as? Int, (1...1920).contains(width), (1...1080).contains(height) else { throw NativeFailure.invalid }
            let image = try await withTaskCancellationHandler(operation: {
                try await withCheckedThrowingContinuation { continuation in
                    photoWaiter = continuation
                    let controller = UIImagePickerController(); controller.sourceType = .camera; controller.delegate = self; picker = controller
                    presenter.present(controller, animated: true)
                }
            }, onCancel: { [weak self] in Task { @MainActor in self?.cancelPhoto() } })
            let ratio = min(1, min(CGFloat(width) / image.size.width, CGFloat(height) / image.size.height))
            let size = CGSize(width: image.size.width * ratio, height: image.size.height * ratio)
            let resized = UIGraphicsImageRenderer(size: size).image { _ in image.draw(in: CGRect(origin: .zero, size: size)) }
            guard let data = resized.jpegData(compressionQuality: 0.7), data.count <= 262_144 else { throw NativeFailure.failed }
            return ["mime": "image/jpeg", "data": Array(data)]
        default: throw NativeFailure.unsupported
        }
    }
    private func cancelPhoto() { photoWaiter?.resume(throwing: CancellationError()); photoWaiter = nil; picker?.dismiss(animated: false); picker = nil }
    public func imagePickerControllerDidCancel(_ picker: UIImagePickerController) { cancelPhoto() }
    public func imagePickerController(_ picker: UIImagePickerController, didFinishPickingMediaWithInfo info: [UIImagePickerController.InfoKey: Any]) {
        let waiter = photoWaiter; photoWaiter = nil; self.picker = nil; picker.dismiss(animated: true)
        if let image = info[.originalImage] as? UIImage { waiter?.resume(returning: image) } else { waiter?.resume(throwing: NativeFailure.failed) }
    }
    nonisolated private static func fileOperation(root: URL, path: String, contents: String?) throws -> String {
        let parts = path.split(separator: "/", omittingEmptySubsequences: false)
        guard !parts.isEmpty, parts.allSatisfy({ !$0.isEmpty && $0 != "." && $0 != ".." && !$0.contains("\0") }) else { throw NativeFailure.invalid }
        var directory = Darwin.open(root.path, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)
        guard directory >= 0 else { throw NativeFailure.failed }; defer { Darwin.close(directory) }
        for part in parts.dropLast() {
            let next = part.withCString { openat(directory, $0, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC) }
            guard next >= 0 else { throw NativeFailure.denied }; Darwin.close(directory); directory = next
        }
        let flags = contents == nil ? O_RDONLY : O_WRONLY | O_CREAT
        let fd = String(parts.last!).withCString { openat(directory, $0, flags | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC, S_IRUSR | S_IWUSR) }
        guard fd >= 0 else { throw NativeFailure.failed }; defer { Darwin.close(fd) }
        var metadata = stat(); guard fstat(fd, &metadata) == 0, metadata.st_mode & S_IFMT == S_IFREG else { throw NativeFailure.denied }
        if let contents {
            let data = Data(contents.utf8); guard data.count <= 262_144, ftruncate(fd, 0) == 0 else { throw NativeFailure.invalid }
            try data.withUnsafeBytes { buffer in
                var offset = 0
                while offset < buffer.count { let count = Darwin.write(fd, buffer.baseAddress!.advanced(by: offset), buffer.count - offset); guard count > 0 else { throw NativeFailure.failed }; offset += count }
            }; return ""
        }
        var bytes = [UInt8](repeating: 0, count: 262_145); var offset = 0
        while offset < bytes.count { let count = bytes.withUnsafeMutableBytes { Darwin.read(fd, $0.baseAddress!.advanced(by: offset), $0.count - offset) }; guard count >= 0 else { throw NativeFailure.failed }; if count == 0 { break }; offset += count }
        guard offset <= 262_144, let text = String(bytes: bytes[..<offset], encoding: .utf8) else { throw NativeFailure.invalid }; return text
    }
    private func failure(_ id: String, code: String) { send(["type": "error", "api_version": 1, "id": id, "error": ["code": code, "message": "Native operation failed"]]) }
    private func send(_ message: [String: Any]) {
        guard !closed, let bytes = try? JSONSerialization.data(withJSONObject: message) else { return }
        guard bytes.count <= 1_048_576 else { if let id = message["id"] as? String { failure(id, code: "invalid_request") }; return }
        if let responseSink {responseSink(bytes);return}
        bytes.withUnsafeBytes { _ = omen_bridge_receive(handle, $0.bindMemory(to: UInt8.self).baseAddress, $0.count) }
    }
}
