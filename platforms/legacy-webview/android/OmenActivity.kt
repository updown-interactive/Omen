package dev.omen.app

import android.annotation.SuppressLint
import android.graphics.Color
import android.net.Uri
import android.os.Bundle
import android.webkit.*
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.webkit.WebViewAssetLoader
import dev.omen.nativebridge.OmenBridge
import kotlinx.coroutines.*
import org.json.JSONObject
import java.io.ByteArrayInputStream

/** Native Android window/compositor/input host for the shared Rust/WASM Omen application. */
class OmenActivity : ComponentActivity() {
    private lateinit var web: WebView
    private lateinit var native: OmenBridge
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private var ready = false
    private var paused = false
    private var destroyed = false
    private val pending = ArrayDeque<String>()
    private var metrics = "{}"
    private val trustedOrigin = "https://appassets.androidplatform.net"

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(state: Bundle?) {
        super.onCreate(state)
        WindowCompat.setDecorFitsSystemWindows(window, false)
        val config = JSONObject(assets.open("omen/app.json").bufferedReader().use { it.readText() })
        val permissions = config.optJSONArray("permissions")
        val allowed = if (permissions == null) emptySet() else (0 until permissions.length()).map { permissions.getString(it) }.toSet()
        native = OmenBridge(this, allowed) { bytes ->
            runOnUiThread {
                if (!destroyed) {
                    val message = "window.OmenHost?.receiveNative(${JSONObject.quote(bytes.toString(Charsets.UTF_8))})"
                    if (ready && !paused) web.evaluateJavascript(message, null) else if (pending.size < 32) pending.addLast(message)
                }
            }
        }
        web = WebView(this)
        web.setBackgroundColor(Color.rgb(16,24,39))
        web.settings.javaScriptEnabled = true
        web.settings.domStorageEnabled = true
        web.settings.allowFileAccess = false
        web.settings.allowContentAccess = false
        web.settings.mixedContentMode = WebSettings.MIXED_CONTENT_NEVER_ALLOW
        web.settings.mediaPlaybackRequiresUserGesture = true
        web.addJavascriptInterface(NativeMessages(), "OmenNative")
        val loader = WebViewAssetLoader.Builder().addPathHandler("/assets/", WebViewAssetLoader.AssetsPathHandler(this)).build()
        web.webViewClient = object : WebViewClient() {
            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean = !trusted(request.url)
            override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? {
                return if (trusted(request.url)) loader.shouldInterceptRequest(request.url)?.apply { if(request.url.path?.endsWith(".mjs")==true)mimeType="text/javascript";if(request.url.path?.endsWith(".wasm")==true)mimeType="application/wasm" } else WebResourceResponse("text/plain","UTF-8",403,"Blocked",emptyMap(),ByteArrayInputStream(ByteArray(0)))
            }
            override fun onPageFinished(view: WebView, url: String) {
                if (!trusted(Uri.parse(url))) return
                ready = true
                // Boot uses async WASM loading. Queue through a ready promise rather than timing guesses.
                web.evaluateJavascript("window.OmenReady?.then(()=>{window.OmenHost.setNativeMetrics($metrics);window.OmenHost.lifecycle('${if(paused) "background" else "foreground"}');})", null)
                flush()
            }
            override fun onRenderProcessGone(view: WebView, detail: RenderProcessGoneDetail): Boolean {
                ready = false; native.close(); view.destroy(); finish(); return true
            }
        }
        setContentView(web)
        ViewCompat.setOnApplyWindowInsetsListener(web) { _, insets ->
            val safe = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
            val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
            val density = resources.displayMetrics.density
            val visibleHeight = (web.rootView.height - ime.bottom).coerceAtLeast(0) / density
            metrics = JSONObject().put("insets", org.json.JSONArray(listOf(safe.top/density,safe.right/density,if(ime.bottom>0)0f else safe.bottom/density,safe.left/density))).put("height", visibleHeight).toString()
            if (ready && !destroyed) web.evaluateJavascript("window.OmenHost?.setNativeMetrics($metrics)",null)
            insets
        }
        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                web.evaluateJavascript("window.OmenHost?.back() ?? false") { handled -> if (handled != "true") finish() }
            }
        })
        web.loadUrl("$trustedOrigin/assets/omen/index.html")
    }
    private fun trusted(url: Uri): Boolean = url.scheme == "https" && url.host == "appassets.androidplatform.net" && url.path?.startsWith("/assets/omen/") == true
    private fun flush() { if (ready && !paused) while (pending.isNotEmpty()) web.evaluateJavascript(pending.removeFirst(),null) }
    override fun onPause() {
        paused = true
        // A camera/permission Activity belongs to this explicit native operation; preserve its pending future.
        if (ready && !native.presentsNativeUi) web.evaluateJavascript("window.OmenHost?.lifecycle('background')",null)
        web.onPause(); super.onPause()
    }
    override fun onResume() { super.onResume(); if (::web.isInitialized) {paused=false;web.onResume();if(ready)web.evaluateJavascript("window.OmenHost?.lifecycle('foreground')",null);flush()} }
    override fun onDestroy() {
        destroyed=true;scope.cancel()
        if (::web.isInitialized) {web.evaluateJavascript("window.OmenHost?.dispose()",null);web.removeJavascriptInterface("OmenNative");web.destroy()}
        if (::native.isInitialized) native.close()
        pending.clear();super.onDestroy()
    }
    inner class NativeMessages {
        @JavascriptInterface fun postMessage(json: String) {
            if (json.toByteArray().size > 1048576) return
            runOnUiThread {
                if (destroyed || !trusted(Uri.parse(web.url ?: ""))) return@runOnUiThread
                val message = try { JSONObject(json) } catch (_: Exception) { return@runOnUiThread }
                when (message.optString("kind")) {
                    "plugin" -> native.dispatch(message.getJSONObject("frame").toString().toByteArray())
                    "permission" -> {
                        val id=message.optString("id");val permission=message.optString("permission")
                        if (!id.matches(Regex("[0-9]{1,20}"))) return@runOnUiThread
                        scope.launch {
                            val status=try{native.requestPermission(permission)}catch(_:Exception){"denied"}
                            if (!destroyed) {val result="window.OmenHost?.permissionResult(${JSONObject.quote(id)},${JSONObject.quote(status)})";if(ready&&!paused)web.evaluateJavascript(result,null)else if(pending.size<32)pending.addLast(result)}
                        }
                    }
                }
            }
        }
    }
}
