package com.mandalsuraj.hushpen.ime

import android.content.Intent
import android.inputmethodservice.InputMethodService
import android.text.InputType
import android.view.View
import android.view.inputmethod.EditorInfo
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.platform.ComposeView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.LifecycleRegistry
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.ViewModelStore
import androidx.lifecycle.ViewModelStoreOwner
import androidx.lifecycle.setViewTreeLifecycleOwner
import androidx.lifecycle.setViewTreeViewModelStoreOwner
import androidx.savedstate.SavedStateRegistry
import androidx.savedstate.SavedStateRegistryController
import androidx.savedstate.SavedStateRegistryOwner
import androidx.savedstate.setViewTreeSavedStateRegistryOwner
import com.mandalsuraj.hushpen.MainActivity
import com.mandalsuraj.hushpen.PhoneApplication
import com.mandalsuraj.hushpen.ui.HushpenTheme

class HushpenInputMethod : InputMethodService(), LifecycleOwner, ViewModelStoreOwner, SavedStateRegistryOwner {
    private val life = LifecycleRegistry(this)
    private val saved = SavedStateRegistryController.create(this)
    override val lifecycle: Lifecycle get() = life
    override val viewModelStore = ViewModelStore()
    override val savedStateRegistry: SavedStateRegistry get() = saved.savedStateRegistry
    private lateinit var model: KeyboardViewModel
    private var receipt: InsertReceipt? = null
    private var selectionStart = -1
    private var selectionEnd = -1
    private val enterKey = mutableStateOf(EnterKey.Return)

    override fun onCreate() {
        super.onCreate()
        saved.performAttach()
        saved.performRestore(null)
        life.handleLifecycleEvent(Lifecycle.Event.ON_CREATE)
        val repository = (application as PhoneApplication).repository
        model = ViewModelProvider(this, object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(modelClass: Class<T>): T = KeyboardViewModel(applicationContext, repository) as T
        })[KeyboardViewModel::class.java]
    }

    override fun onCreateInputView(): View {
        checkNotNull(window.window).decorView.apply {
            setViewTreeLifecycleOwner(this@HushpenInputMethod)
            setViewTreeViewModelStoreOwner(this@HushpenInputMethod)
            setViewTreeSavedStateRegistryOwner(this@HushpenInputMethod)
        }
        return ComposeView(this).apply {
            val actions = KeyboardActions(::type, ::backspace, ::enter, ::undo, ::openApp)
            setContent { HushpenTheme { HushpenKeyboard(model, enterKey.value, actions) } }
        }
    }

    override fun onStartInput(attribute: EditorInfo, restarting: Boolean) {
        super.onStartInput(attribute, restarting)
        receipt = null
        selectionStart = attribute.initialSelStart
        selectionEnd = attribute.initialSelEnd
        model.openInput(allowsDictation(attribute), ::insertDictation)
    }

    private fun allowsDictation(attribute: EditorInfo): Boolean {
        val inputClass = attribute.inputType and InputType.TYPE_MASK_CLASS
        val variation = attribute.inputType and InputType.TYPE_MASK_VARIATION
        val password = (inputClass == InputType.TYPE_CLASS_TEXT && variation in setOf(
            InputType.TYPE_TEXT_VARIATION_PASSWORD, InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD,
            InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD,
        )) || (inputClass == InputType.TYPE_CLASS_NUMBER && variation == InputType.TYPE_NUMBER_VARIATION_PASSWORD)
        return !password
    }

    override fun onStartInputView(info: EditorInfo, restarting: Boolean) {
        super.onStartInputView(info, restarting)
        enterKey.value = EnterKey.of(info)
        if (!model.inputOpen) {
            model.openInput(allowsDictation(info), ::insertDictation)
        }
        life.handleLifecycleEvent(Lifecycle.Event.ON_START)
        life.handleLifecycleEvent(Lifecycle.Event.ON_RESUME)
        updateShift()
    }

    override fun onFinishInputView(finishingInput: Boolean) {
        if (life.currentState.isAtLeast(Lifecycle.State.RESUMED)) life.handleLifecycleEvent(Lifecycle.Event.ON_PAUSE)
        if (life.currentState.isAtLeast(Lifecycle.State.STARTED)) life.handleLifecycleEvent(Lifecycle.Event.ON_STOP)
        model.closeInput()
        receipt = null
        super.onFinishInputView(finishingInput)
    }

    override fun onFinishInput() {
        model.closeInput()
        receipt = null
        super.onFinishInput()
    }

    override fun onUpdateSelection(oldSelStart: Int, oldSelEnd: Int, newSelStart: Int, newSelEnd: Int, candidatesStart: Int, candidatesEnd: Int) {
        super.onUpdateSelection(oldSelStart, oldSelEnd, newSelStart, newSelEnd, candidatesStart, candidatesEnd)
        selectionStart = newSelStart
        selectionEnd = newSelEnd
        if (receipt?.let { newSelStart != it.end || newSelEnd != it.end } == true) receipt = null
        model.setUndo(receipt != null)
        if (newSelStart == newSelEnd) updateShift()
    }

    private fun updateShift() {
        val info = currentInputEditorInfo ?: return
        val connection = currentInputConnection ?: return
        model.autoShift(info.inputType != InputType.TYPE_NULL && connection.getCursorCapsMode(info.inputType) != 0)
    }

    private fun type(text: String) {
        receipt = null
        currentInputConnection?.commitText(text, 1)
        model.typed()
    }

    private fun backspace() {
        receipt = null
        val connection = currentInputConnection ?: return
        if (!connection.getSelectedText(0).isNullOrEmpty()) connection.commitText("", 1)
        else connection.deleteSurroundingTextInCodePoints(1, 0)
        model.setUndo(false)
    }

    private fun enter() {
        receipt = null
        val info = currentInputEditorInfo ?: return
        val action = info.imeOptions and EditorInfo.IME_MASK_ACTION
        val allowAction = info.imeOptions and EditorInfo.IME_FLAG_NO_ENTER_ACTION == 0
        when {
            allowAction && !info.actionLabel.isNullOrEmpty() -> currentInputConnection?.performEditorAction(info.actionId)
            allowAction && action != EditorInfo.IME_ACTION_NONE && action != EditorInfo.IME_ACTION_UNSPECIFIED -> currentInputConnection?.performEditorAction(action)
            else -> currentInputConnection?.commitText("\n", 1)
        }
        model.setUndo(false)
    }

    private fun insertDictation(text: String): Boolean {
        val connection = currentInputConnection ?: return false
        val before = connection.getTextBeforeCursor(32, 0)?.toString() ?: ""
        val replaced = connection.getSelectedText(0)?.toString()
        val start = minOf(selectionStart, selectionEnd)
        receipt = null
        if (!connection.commitText(text, 1)) return false
        if (start >= 0 && text.isNotEmpty() && (selectionStart == selectionEnd || replaced != null)) {
            receipt = InsertReceipt(text, start + text.length, before, replaced.orEmpty())
            selectionStart = start + text.length
            selectionEnd = selectionStart
        }
        model.setUndo(receipt != null)
        return true
    }

    private fun undo() {
        val inserted = receipt ?: return
        val connection = currentInputConnection ?: return
        val actual = connection.getTextBeforeCursor(inserted.before.length + inserted.text.length, 0)?.toString() ?: ""
        receipt = null
        if (inserted.matches(selectionStart, selectionEnd, actual)) {
            connection.beginBatchEdit()
            try {
                if (connection.setSelection(inserted.end - inserted.text.length, inserted.end)) {
                    connection.commitText(inserted.replaced, 1)
                }
            } finally {
                connection.endBatchEdit()
            }
        } else {
            model.notice("The text changed. Undo is unavailable.")
        }
        model.setUndo(false)
    }

    private fun openApp() {
        startActivity(Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        requestHideSelf(0)
    }

    override fun onDestroy() {
        viewModelStore.clear()
        life.handleLifecycleEvent(Lifecycle.Event.ON_DESTROY)
        super.onDestroy()
    }
}
