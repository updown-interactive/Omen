package dev.omen.app

import android.content.res.Configuration
import android.os.Build
import android.os.Bundle
import android.view.WindowInsets

/** GPUI owns the native window, event loop, graphics surface and input. */
class OmenActivity : android.app.NativeActivity() {
    companion object {
        @JvmStatic
        @Volatile
        private var insets: IntArray = intArrayOf(0, 0, 0, 0)

        @JvmStatic
        fun getSafeInsets(): IntArray = insets

        @Volatile
        private var deviceSettings = floatArrayOf(1f, 0f, 0f, 0f, 0f, 0f)

        @JvmStatic
        fun getDeviceSettings(): FloatArray = deviceSettings
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val resourceId = resources.getIdentifier("status_bar_height", "dimen", "android")
        val statusBarHeight = if (resourceId > 0) resources.getDimensionPixelSize(resourceId) else 0
        insets = intArrayOf(statusBarHeight, 0, 0, 0)
        updateDeviceSettings(resources.configuration, null)

        window.decorView.setOnApplyWindowInsetsListener { _, windowInsets ->
            updateInsets(windowInsets)
            windowInsets
        }
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        updateDeviceSettings(newConfig, window.decorView.rootWindowInsets)
        super.onConfigurationChanged(newConfig)
    }

    private fun updateDeviceSettings(config: Configuration, windowInsets: WindowInsets?) {
        val dark = (config.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES
        val ime = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            windowInsets?.getInsets(WindowInsets.Type.ime())
        } else null
        deviceSettings = floatArrayOf(
            config.fontScale, if (dark) 1f else 0f,
            (ime?.top ?: 0).toFloat(), (ime?.right ?: 0).toFloat(),
            (ime?.bottom ?: 0).toFloat(), (ime?.left ?: 0).toFloat()
        )
    }

    private fun updateInsets(windowInsets: WindowInsets) {
        updateDeviceSettings(resources.configuration, windowInsets)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            val bars = windowInsets.getInsets(WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout())
            insets = intArrayOf(bars.top, bars.right, bars.bottom, bars.left)
        } else {
            @Suppress("DEPRECATION")
            insets = intArrayOf(
                windowInsets.systemWindowInsetTop,
                windowInsets.systemWindowInsetRight,
                windowInsets.systemWindowInsetBottom,
                windowInsets.systemWindowInsetLeft
            )
        }
    }
}

