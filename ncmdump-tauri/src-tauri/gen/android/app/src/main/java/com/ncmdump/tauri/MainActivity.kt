package com.ncmdump.tauri

import android.os.Bundle
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  private external fun nativeInitAndroidContext()
  private external fun nativeReleaseAndroidContext()

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    nativeInitAndroidContext()
  }

  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    webView.settings.userAgentString =
      "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 " +
        "(KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
    webView.settings.setSupportZoom(true)
    webView.settings.builtInZoomControls = true
    webView.settings.displayZoomControls = false
    webView.settings.useWideViewPort = true
    webView.settings.loadWithOverviewMode = true
  }

  override fun onDestroy() {
    nativeReleaseAndroidContext()
    super.onDestroy()
  }
}
