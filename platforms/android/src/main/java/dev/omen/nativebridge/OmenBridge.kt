package dev.omen.nativebridge

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.os.Build
import android.system.Os
import android.system.OsConstants
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import kotlinx.coroutines.*
import org.json.JSONObject
import org.json.JSONArray
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import kotlin.coroutines.resume

/** Construct before Activity STARTED. Only app-allowlisted capabilities can run. */
class OmenBridge(private val activity: ComponentActivity, private val allowlist: Set<String>, private val responseSink: ((ByteArray) -> Unit)? = null) : AutoCloseable {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val calls = mutableMapOf<String, Job>()
    private var closed = false
    @JvmField var nativeContext: Long = 0
    val handle: Long
    private var permissionWaiter: CancellableContinuation<Boolean>? = null
    private var permissionBusy = false
    private var cameraWaiter: CancellableContinuation<Bitmap?>? = null
    private var cameraBusy = false
    val presentsNativeUi: Boolean get() = cameraBusy || permissionBusy
    private val permissionLauncher = activity.registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        permissionBusy = false
        permissionWaiter?.let { if (it.isActive) it.resume(granted) }; permissionWaiter = null
    }
    private val cameraLauncher = activity.registerForActivityResult(ActivityResultContracts.TakePicturePreview()) { bitmap ->
        cameraBusy = false
        cameraWaiter?.let { if (it.isActive) it.resume(bitmap) }; cameraWaiter = null
    }
    init { System.loadLibrary("omen_android"); handle = nativeCreate(); check(handle != 0L) }
    private external fun nativeCreate(): Long
    private external fun nativeReceive(handle: Long, data: ByteArray): Boolean
    private external fun nativeDestroy(handle: Long, context: Long)

    /** JNI calls this from any Rust thread; all lifecycle/UI work is marshalled to Main. */
    fun dispatch(bytes: ByteArray) {
        if (bytes.size > 1048576) return
        scope.launch {
            if (closed) return@launch
            val message = try { JSONObject(bytes.toString(Charsets.UTF_8)) } catch (_: Exception) { return@launch }
            if (message.optInt("api_version") != 1) return@launch
            val id = message.optString("id")
            if (!id.matches(Regex("[0-9]{1,20}"))) return@launch
            if (message.optString("type") == "cancel") { calls.remove(id)?.cancel(); return@launch }
            if (message.optString("type") != "call" || calls.containsKey(id)) return@launch
            if (calls.size >= 256) { failure(id, "unavailable", "Too many native calls"); return@launch }
            val job = scope.launch(start = CoroutineStart.LAZY) {
                try {
                    val plugin = message.getString("plugin"); val operation = message.getString("operation")
                    val payload = message.getJSONObject("payload")
                    val output = invoke(plugin, operation, payload)
                    response(id, output)
                } catch (_: CancellationException) { /* Rust no longer expects a response. */ }
                  catch (_: SecurityException) { failure(id, "permission_denied", "Native permission not granted") }
                  catch (_: IllegalArgumentException) { failure(id, "invalid_request", "Invalid native request") }
                  catch (_: UnsupportedOperationException) { failure(id, "unsupported", "Native capability unavailable") }
                  catch (_: Exception) { failure(id, "native", "Native operation failed") }
                finally { calls.remove(id) }
            }
            calls[id] = job; job.start()
        }
    }
    fun permissionStatus(permission: String): String {
        if (!allowlist.contains(permission)) return "denied"
        return when (permission) {
            "fs.read", "fs.write" -> "granted"
            "camera" -> if (activity.checkSelfPermission(Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) "granted" else "prompt"
            "notifications" -> if ((Build.VERSION.SDK_INT < 33 || activity.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED) && activity.getSystemService(NotificationManager::class.java).areNotificationsEnabled()) "granted" else "prompt"
            else -> "denied"
        }
    }
    /** Call from a user action; propagate the result to the Rust host permission store. */
    suspend fun requestPermission(permission: String): String = withContext(Dispatchers.Main.immediate) {
        if (!allowlist.contains(permission)) return@withContext "denied"
        if (permissionStatus(permission) == "granted") return@withContext "granted"
        val native = when (permission) {
            "camera" -> Manifest.permission.CAMERA
            "notifications" -> if (Build.VERSION.SDK_INT >= 33) Manifest.permission.POST_NOTIFICATIONS else return@withContext "denied"
            else -> return@withContext "denied"
        }
        check(!permissionBusy) { "A permission request is already active" }; permissionBusy = true
        val granted = suspendCancellableCoroutine { continuation: CancellableContinuation<Boolean> ->
            permissionWaiter = continuation
            continuation.invokeOnCancellation { activity.runOnUiThread { permissionWaiter = null } }
            permissionLauncher.launch(native)
        }
        if (granted) "granted" else "denied"
    }
    private fun requirePermission(permission: String) { if (permissionStatus(permission) != "granted") throw SecurityException() }
    private suspend fun invoke(plugin: String, operation: String, payload: JSONObject): Any {
        return when ("$plugin/$operation") {
            "omen.filesystem/read_text" -> withContext(Dispatchers.IO) {
                requirePermission("fs.read")
                val fd = Os.open(file(payload.getString("path")).path, OsConstants.O_RDONLY or OsConstants.O_NOFOLLOW or OsConstants.O_NONBLOCK, 0)
                FileInputStream(fd).use { stream ->
                    require(Os.fstat(fd).st_mode and OsConstants.S_IFMT == OsConstants.S_IFREG)
                    val buffer = ByteArray(262145); var length = 0
                    while (length < buffer.size) { val count = stream.read(buffer, length, buffer.size - length); if (count < 0) break; length += count }; require(length <= 262144); val bytes = buffer.copyOf(length)
                    Charsets.UTF_8.newDecoder().decode(java.nio.ByteBuffer.wrap(bytes)).toString()
                }
            }
            "omen.filesystem/write_text" -> withContext(Dispatchers.IO) {
                val path = payload.getString("path"); val bytes = payload.getString("contents").toByteArray(Charsets.UTF_8); require(bytes.size <= 262144)
                requirePermission("fs.write")
                val fd = Os.open(file(path).path, OsConstants.O_WRONLY or OsConstants.O_CREAT or OsConstants.O_NOFOLLOW or OsConstants.O_NONBLOCK, 384)
                FileOutputStream(fd).use { stream ->
                    require(Os.fstat(fd).st_mode and OsConstants.S_IFMT == OsConstants.S_IFREG)
                    requirePermission("fs.write"); Os.ftruncate(fd, 0); stream.write(bytes)
                }
                emit("omen.filesystem", "changed", JSONObject().put("path", path)); JSONObject.NULL
            }
            "omen.notifications/show" -> {
                requirePermission("notifications")
                val title = payload.getString("title"); val body = payload.getString("body"); require(title.toByteArray().size <= 256 && body.toByteArray().size <= 4096)
                val manager = activity.getSystemService(NotificationManager::class.java)
                manager.createNotificationChannel(NotificationChannel("omen", "Omen", NotificationManager.IMPORTANCE_DEFAULT))
                manager.notify(1, android.app.Notification.Builder(activity, "omen").setSmallIcon(android.R.drawable.ic_dialog_info).setContentTitle(title).setContentText(body).build()); JSONObject.NULL
            }
            "omen.camera/capture" -> {
                requirePermission("camera")
                val width = payload.getInt("width"); val height = payload.getInt("height"); require(width in 1..1920 && height in 1..1080)
                check(!cameraBusy); cameraBusy = true
                val bitmap = suspendCancellableCoroutine { continuation: CancellableContinuation<Bitmap?> ->
                    cameraWaiter = continuation; continuation.invokeOnCancellation { activity.runOnUiThread { cameraWaiter = null } }; cameraLauncher.launch(null)
                } ?: throw IllegalStateException("Camera cancelled")
                val scaled = Bitmap.createScaledBitmap(bitmap, minOf(width, bitmap.width), minOf(height, bitmap.height), true)
                val output = ByteArrayOutputStream(); scaled.compress(Bitmap.CompressFormat.JPEG, 75, output); val bytes = output.toByteArray(); require(bytes.size <= 262144)
                JSONObject().put("mime", "image/jpeg").put("data", JSONArray(bytes.map { it.toInt() and 255 }))
            }
            else -> throw UnsupportedOperationException()
        }
    }
    // Private app storage, single filename only; no intermediate-directory symlink traversal.
    private fun file(name: String): File { require(name.isNotEmpty() && name != "." && name != ".." && !name.contains('/') && !name.contains('\\')); return File(activity.filesDir, name) }
    private fun response(id: String, output: Any) = send(JSONObject().put("type", "response").put("api_version", 1).put("id", id).put("payload", output))
    private fun failure(id: String, code: String, message: String) { send(JSONObject().put("type", "error").put("api_version", 1).put("id", id).put("error", JSONObject().put("code", code).put("message", message))) }
    private fun emit(plugin: String, topic: String, payload: JSONObject) = send(JSONObject().put("type", "event").put("api_version", 1).put("plugin", plugin).put("topic", topic).put("payload", payload))
    private fun send(message: JSONObject) {
        if (closed) return
        val bytes = message.toString().toByteArray(Charsets.UTF_8)
        if (bytes.size <= 1048576) { if (responseSink != null) responseSink.invoke(bytes) else nativeReceive(handle, bytes) }
        else if (message.has("id")) failure(message.getString("id"), "invalid_request", "Response exceeds bridge limit")
    }
    override fun close() { if (closed) return; closed = true; scope.cancel(); nativeDestroy(handle, nativeContext) }
}
