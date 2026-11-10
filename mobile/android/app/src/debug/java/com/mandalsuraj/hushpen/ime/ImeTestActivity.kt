package com.mandalsuraj.hushpen.ime

import android.os.Bundle
import android.text.InputType
import android.view.Window
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.lifecycle.lifecycleScope
import com.mandalsuraj.hushpen.PhoneApplication
import com.mandalsuraj.hushpen.core.HostUnloadRule
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.TimeoutCancellationException
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout
import java.io.File

class ImeTestActivity : ComponentActivity() {
    lateinit var editor: EditText
        private set
    lateinit var otherEditor: EditText
        private set
    var editorAction: Int? = null
        private set

    override fun onCreate(savedInstanceState: Bundle?) {
        requestWindowFeature(Window.FEATURE_NO_TITLE)
        super.onCreate(savedInstanceState)
        editor = EditText(this).apply {
            id = 12341
            hint = "IME test editor"
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE
            imeOptions = EditorInfo.IME_ACTION_NONE or EditorInfo.IME_FLAG_NO_ENTER_ACTION
            setOnEditorActionListener { _, action, _ -> editorAction = action; true }
        }
        otherEditor = EditText(this).apply {
            id = 12342
            hint = "Other test editor"
        }
        val status = TextView(this)
        setContentView(LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            fitsSystemWindows = true
            addView(status)
            addView(editor)
            addView(otherEditor)
        })
        editor.requestFocus()
        intent.getStringExtra("pairingUri")?.let { uri ->
            lifecycleScope.launch {
                val repository = (application as PhoneApplication).repository
                var stage = "initialize"
                try {
                    withTimeout(150_000) {
                        repository.initialize()
                        stage = "verify model"
                        check(repository.verify("base-q5_1")) { "The test model is missing." }
                        repository.selectModel("base-q5_1")
                        stage = "start server"
                        repository.serverController.start(HostUnloadRule.KeepLoaded)
                        val serving = withTimeout(10_000) {
                            repository.serverController.state.first { it.running || it.notice != null }
                        }
                        check(serving.running) { "Server mode did not start." }
                        stage = "pair"
                        repository.pair(uri)
                        val ready = "Server ready: ${serving.port}"
                        status.text = ready
                        withContext(Dispatchers.IO) { File(filesDir, "android-server-status").writeText(ready) }
                    }
                } catch (error: Exception) {
                    if (error is CancellationException && error !is TimeoutCancellationException) throw error
                    val failed = "Server setup failed during $stage."
                    status.text = failed
                    withContext(Dispatchers.IO) { File(filesDir, "android-server-status").writeText(failed) }
                }
            }
        }
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus && ::editor.isInitialized) {
            editor.post {
                if (editor.hasWindowFocus() && editor.hasFocus()) {
                    getSystemService(InputMethodManager::class.java).showSoftInput(editor, InputMethodManager.SHOW_IMPLICIT)
                }
            }
        }
    }

    override fun onResume() {
        super.onResume()
        (application as PhoneApplication).repository.setActivityVisible(true)
    }

    override fun onStop() {
        (application as PhoneApplication).repository.setActivityVisible(false)
        super.onStop()
    }
}
