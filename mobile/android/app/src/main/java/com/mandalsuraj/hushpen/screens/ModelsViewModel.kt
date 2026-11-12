package com.mandalsuraj.hushpen.screens

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.mandalsuraj.hushpen.DownloadState
import com.mandalsuraj.hushpen.core.Model
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch

interface ModelsData {
    val models: StateFlow<List<Model>>
    val selectedModel: StateFlow<String>
    val selectedCleanupModel: StateFlow<String>
    val download: StateFlow<DownloadState>
    suspend fun initialize()
    suspend fun refresh()
    suspend fun download(id: String)
    suspend fun verify(id: String): Boolean
    suspend fun selectModel(id: String)
    fun cancelDownload()
}

class ModelsViewModel(private val repository: ModelsData) : ViewModel() {
    val models = repository.models
    val selectedModel = repository.selectedModel
    val selectedCleanupModel = repository.selectedCleanupModel
    val download = repository.download
    private val mutableNotice = MutableStateFlow<String?>(null)
    val notice: StateFlow<String?> = mutableNotice

    init { refresh() }

    fun refresh() = run { repository.refresh() }
    fun download(id: String) = run { repository.download(id) }
    fun verify(id: String) = run {
        if (!repository.verify(id)) mutableNotice.value = "The model did not pass verification."
    }
    fun select(id: String) = run { repository.selectModel(id) }
    fun cancelDownload() = repository.cancelDownload()

    override fun onCleared() {
        repository.cancelDownload()
        super.onCleared()
    }

    private fun run(action: suspend () -> Unit) = viewModelScope.launch {
        mutableNotice.value = null
        try {
            repository.initialize()
            action()
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (problem: Exception) {
            mutableNotice.value = problem.message ?: "The model action failed."
        }
    }
}
