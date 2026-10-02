package dev.framemate.app

import android.content.pm.ActivityInfo
import android.os.Bundle
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }

  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    webView.addJavascriptInterface(FullscreenBridge(), "FrameMateAndroid")
  }

  /**
   * The app is portrait-only (see AndroidManifest). The headset view calls
   * `FrameMateAndroid.setFullscreen(true)` to go landscape without system bars.
   * WebViews can't do this themselves: the Screen Orientation API is browser-only.
   */
  inner class FullscreenBridge {
    @JavascriptInterface
    fun setFullscreen(enabled: Boolean) = runOnUiThread {
      requestedOrientation =
        if (enabled) ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
        else ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
      val bars = WindowCompat.getInsetsController(window, window.decorView)
      if (enabled) {
        bars.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        bars.hide(WindowInsetsCompat.Type.systemBars())
      } else {
        bars.show(WindowInsetsCompat.Type.systemBars())
      }
    }
  }
}
