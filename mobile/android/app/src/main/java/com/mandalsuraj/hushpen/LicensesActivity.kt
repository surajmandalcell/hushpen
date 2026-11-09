package com.mandalsuraj.hushpen

import android.app.Activity
import android.content.Context
import android.os.Bundle
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient

class LicensesActivity : Activity() {
    private lateinit var licenses: WebView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        licenses = PlaceholderLicenses(this)
        setContentView(licenses)
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
