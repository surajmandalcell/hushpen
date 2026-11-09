package com.mandalsuraj.hushpen.server

import com.google.mlkit.genai.common.FeatureStatus
import com.google.mlkit.genai.prompt.Generation
import com.google.mlkit.genai.prompt.TextPart
import com.google.mlkit.genai.prompt.generateContentRequest
import com.mandalsuraj.hushpen.core.MobileException
import com.mandalsuraj.hushpen.core.SystemLlm
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout

interface SystemPrompt {
    fun available(): Boolean
    fun generate(text: String, instructions: String): String
}

class NativeSystemLlm(private val prompt: SystemPrompt = GeminiPrompt()) : SystemLlm {
    @Volatile var enabled = true
    @Volatile var foreground = false

    override fun available(): Boolean = enabled && foreground && runCatching { prompt.available() }.getOrDefault(false)

    override fun cleanup(text: String, instructions: String): String {
        if (!enabled) throw MobileException.Failed("System cleanup is off")
        if (!foreground) throw MobileException.Failed("Keep Hushpen open to use system cleanup")
        return try {
            prompt.generate(text, instructions)
        } catch (problem: Exception) {
            throw MobileException.Failed(problem.message ?: "System cleanup failed")
        }
    }
}

private class GeminiPrompt : SystemPrompt {
    private val model by lazy { Generation.getClient() }

    override fun available(): Boolean = runBlocking {
        withTimeout(1_500) { model.checkStatus() == FeatureStatus.AVAILABLE }
    }

    override fun generate(text: String, instructions: String): String = runBlocking {
        withTimeout(28_000) {
            val request = generateContentRequest(TextPart("$instructions\n\n$text")) {
                temperature = 0.0f
                candidateCount = 1
                maxOutputTokens = 2048
            }
            val response = model.generateContent(request)
            response.candidates.firstOrNull()?.text?.takeIf { it.isNotBlank() }
                ?: throw MobileException.Failed("System cleanup returned no text")
        }
    }
}
