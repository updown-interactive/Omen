package dev.omen.app
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import android.os.SystemClock
import android.os.ParcelFileDescriptor
import android.os.Process
import android.graphics.Bitmap
import java.io.File
import org.junit.Test
import org.junit.Assert.*
import org.junit.runner.RunWith

/** Startup smoke test only; GPUI pixel and input assertions require device qualification. */
@RunWith(AndroidJUnit4::class)
class OmenAppTest {
    @Test fun nativeActivityCreatesAWindow() {
        ActivityScenario.launch(OmenActivity::class.java).use { scenario ->
            scenario.onActivity { activity ->
                assertTrue(activity is android.app.NativeActivity)
                assertNotNull(activity.window.decorView)
            }
            val automation = InstrumentationRegistry.getInstrumentation().uiAutomation
            val deadline = SystemClock.elapsedRealtime() + 20000
            var presented = false
            while (SystemClock.elapsedRealtime() < deadline) {
                val logs = automation.executeShellCommand("logcat -d --pid=${Process.myPid()} -s gpui_android:I")
                val text = ParcelFileDescriptor.AutoCloseInputStream(logs).bufferedReader().use { it.readText() }
                if (text.contains("GPUI Vulkan frame presented")) { presented = true; break }
                SystemClock.sleep(100)
            }
            assertTrue("GPUI must present a Vulkan frame", presented)
            SystemClock.sleep(500)
            val screenshot = automation.takeScreenshot()
            assertNotNull("Native GPU screen capture", screenshot)
            scenario.onActivity { activity ->
                File(activity.cacheDir, "native-gpui.png").outputStream().use {
                    screenshot.compress(Bitmap.CompressFormat.PNG, 100, it)
                }
            }
        }
    }
}
