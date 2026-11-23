package dev.omen.app
import android.webkit.WebView
import android.view.MotionEvent
import android.os.SystemClock
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Test
import org.junit.Assert.*
import org.junit.runner.RunWith
import org.json.JSONObject
import org.json.JSONTokener
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

@RunWith(AndroidJUnit4::class)
class OmenAppTest {
    private fun evaluate(scenario: ActivityScenario<OmenActivity>, script: String): String {
        val latch=CountDownLatch(1);var result="null"
        scenario.onActivity { activity -> val web=activity.findViewById<android.view.ViewGroup>(android.R.id.content).getChildAt(0) as WebView;web.evaluateJavascript(script){result=it;latch.countDown()} }
        check(latch.await(10,TimeUnit.SECONDS));return result
    }
    private fun waitFor(scenario: ActivityScenario<OmenActivity>, script: String) {
        val deadline=SystemClock.uptimeMillis()+20000
        while(SystemClock.uptimeMillis()<deadline){if(evaluate(scenario,script)=="true")return;Thread.sleep(100)}
        fail("Timed out waiting for bundled WASM UI: ${evaluate(scenario,"document.body.innerText")}")
    }
    private fun tap(scenario: ActivityScenario<OmenActivity>, title: String) {
        val quoted=JSONObject.quote(title)
        val text=JSONTokener(evaluate(scenario,"(()=>{const e=[...document.querySelectorAll('button')].find(e=>e.textContent===$quoted);e.scrollIntoView({block:'center'});const r=e.getBoundingClientRect();return JSON.stringify({x:r.x+r.width/2,y:r.y+r.height/2,width:innerWidth});})()")).nextValue() as String
        val point=JSONObject(text);val latch=CountDownLatch(1);var x=0f;var y=0f
        scenario.onActivity { activity ->val web=activity.findViewById<android.view.ViewGroup>(android.R.id.content).getChildAt(0);val location=IntArray(2);web.getLocationOnScreen(location);val scale=web.width/point.getDouble("width");x=(location[0]+point.getDouble("x")*scale).toFloat();y=(location[1]+point.getDouble("y")*scale).toFloat();latch.countDown() }
        assertTrue(latch.await(5,TimeUnit.SECONDS));val now=SystemClock.uptimeMillis();val instrumentation=InstrumentationRegistry.getInstrumentation()
        val down=MotionEvent.obtain(now,now,MotionEvent.ACTION_DOWN,x,y,0);val up=MotionEvent.obtain(now,now+80,MotionEvent.ACTION_UP,x,y,0)
        instrumentation.sendPointerSync(down);instrumentation.sendPointerSync(up);down.recycle();up.recycle()
    }
    @Test fun rustStateNativeTouchPluginAndLifecycleRoundTrip() {
        ActivityScenario.launch(OmenActivity::class.java).use { scenario ->
            waitFor(scenario,"document.body.innerText.includes('Count: 0')")
            tap(scenario,"Increment");waitFor(scenario,"document.body.innerText.includes('Count: 1')")
            tap(scenario,"Native file round trip");waitFor(scenario,"document.body.innerText.includes('Native file: Hello from Rust')")
            evaluate(scenario,"window.OmenHost.lifecycle('background')");evaluate(scenario,"window.OmenHost.lifecycle('foreground')")
            waitFor(scenario,"document.body.innerText.includes('Count: 1')")
            tap(scenario,"Open dialog");waitFor(scenario,"!!document.querySelector('dialog[open]')")
            evaluate(scenario,"window.OmenHost.back()");waitFor(scenario,"!document.querySelector('dialog[open]')")
            assertEquals("true",evaluate(scenario,"!!document.querySelector('input[aria-label=\"Your name\"]')"))
        }
    }
}
