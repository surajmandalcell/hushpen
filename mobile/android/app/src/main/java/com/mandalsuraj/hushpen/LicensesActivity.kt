package com.mandalsuraj.hushpen

import android.content.Context
import android.os.Build
import android.os.Bundle
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.viewinterop.AndroidView
import com.mandalsuraj.hushpen.ui.HushpenTheme
import com.mandalsuraj.hushpen.ui.LocalSurfaces

class LicensesActivity : ComponentActivity() {
    @OptIn(ExperimentalMaterial3Api::class)
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        setContent {
            HushpenTheme {
                val context = LocalContext.current
                val licenses = remember(context) { licensesView(context) }
                DisposableEffect(licenses) { onDispose { licenses.destroy() } }
                val page = LocalSurfaces.current.page
                Scaffold(
                    containerColor = page,
                    topBar = {
                        TopAppBar(
                            title = { Text("Open-source licenses") },
                            navigationIcon = {
                                IconButton(onClick = ::finish) {
                                    Icon(painterResource(R.drawable.ms_arrow_back), contentDescription = "Back")
                                }
                            },
                            colors = TopAppBarDefaults.topAppBarColors(containerColor = page),
                        )
                    },
                ) { padding ->
                    AndroidView(factory = { licenses }, modifier = Modifier.fillMaxSize().padding(padding))
                }
            }
        }
    }
}

/** Only the bundled notice loads: no scripts, network, or other local files. */
private fun licensesView(context: Context) = WebView(context).apply {
    settings.javaScriptEnabled = false
    settings.allowFileAccess = false
    settings.allowContentAccess = false
    settings.blockNetworkLoads = true
    if (Build.VERSION.SDK_INT >= 33) settings.isAlgorithmicDarkeningAllowed = true
    webViewClient = object : WebViewClient() {
        override fun shouldOverrideUrlLoading(view: WebView?, request: WebResourceRequest?): Boolean =
            request?.url?.toString()?.startsWith("file:///android_asset/") != true
    }
    loadUrl("file:///android_asset/THIRD-PARTY-NOTICES.html")
}
