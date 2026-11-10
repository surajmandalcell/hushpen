package com.mandalsuraj.hushpen

import android.app.Activity
import android.content.Context
import android.os.Bundle
import android.view.Window
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class LicensesActivity : Activity() {
    private lateinit var licenses: WebView

    override fun onCreate(savedInstanceState: Bundle?) {
        requestWindowFeature(Window.FEATURE_NO_TITLE)
        super.onCreate(savedInstanceState)
        licenses = PlaceholderLicenses(this)
        val content = FrameLayout(this).apply {
            addView(licenses, FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT))
        }
        ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
            view.setPadding(bars.left, bars.top, bars.right, bars.bottom)
            insets
        }
        setContentView(content)
    }

    override fun onDestroy() {
        licenses.destroy()
        super.onDestroy()
    }
}

private fun PlaceholderLicenses(context: Context) = WebView(context).apply {
    settings.javaScriptEnabled = false
    settings.allowFileAccess = false
    settings.allowContentAccess = false
    settings.blockNetworkLoads = true
    webViewClient = object : WebViewClient() {
        override fun shouldOverrideUrlLoading(view: WebView?, request: WebResourceRequest?): Boolean =
            request?.url?.toString()?.startsWith("file:///android_asset/") != true
    }
    loadUrl("file:///android_asset/THIRD-PARTY-NOTICES.html")
}
