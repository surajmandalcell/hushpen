package com.mandalsuraj.hushpen.ime

import org.junit.Assert.assertEquals
import org.junit.Test
import java.nio.ByteBuffer
import java.nio.ByteOrder

class MicrophoneTest {
    @Test fun wavHeaderMatchesMono16BitPcm() {
        val bytes = Microphone.wavHeader(32_000)
        assertEquals(44, bytes.size)
        val header = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)
        assertEquals("RIFF", String(bytes, 0, 4, Charsets.US_ASCII))
        assertEquals(32_036, header.getInt(4))
        assertEquals("WAVEfmt ", String(bytes, 8, 8, Charsets.US_ASCII))
        assertEquals(1.toShort(), header.getShort(20))
        assertEquals(1.toShort(), header.getShort(22))
        assertEquals(16_000, header.getInt(24))
        assertEquals(32_000, header.getInt(28))
        assertEquals(2.toShort(), header.getShort(32))
        assertEquals(16.toShort(), header.getShort(34))
        assertEquals(32_000, header.getInt(40))
    }
}
