package com.mandalsuraj.hushpen

import android.app.Application
import android.content.Context
import java.io.IOException
import java.net.URL
import java.net.URLConnection
import java.net.URLStreamHandler
import com.mandalsuraj.hushpen.server.ServerApplication
import com.mandalsuraj.hushpen.server.ServerController

class PhoneApplication : Application(), ServerApplication {
    override fun attachBaseContext(base: Context) {
        super.attachBaseContext(base)
        URL.setURLStreamHandlerFactory { protocol ->
            if (protocol == "http" || protocol == "https") object : URLStreamHandler() {
                override fun openConnection(url: URL): URLConnection =
                    throw IOException("Platform HTTP is disabled. Rust owns network access.")
            } else null
        }
    }

    val repository by lazy { PhoneRepository(this) }
    override val serverController: ServerController
        get() = repository.serverController
}
