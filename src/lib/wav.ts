export async function createTranscriptionAudioPayload(blob: Blob) {
  const wavBlob = await convertBlobToWav(blob).catch(() => blob);

  return {
    audioData: await wavBlob.arrayBuffer(),
    mimeType: wavBlob.type || blob.type || "audio/wav",
  };
}

export function dataUrlToBlob(dataUrl: string) {
  if (!dataUrl.startsWith("data:")) {
    throw new Error("Saved source audio could not be loaded.");
  }

  const commaIndex = dataUrl.indexOf(",");
  if (commaIndex < 0) {
    throw new Error("Saved source audio could not be loaded.");
  }

  const header = dataUrl.slice(5, commaIndex);
  const payload = dataUrl.slice(commaIndex + 1);
  const headerParts = header.split(";").filter(Boolean);
  const mimeType = headerParts[0] || "audio/webm";
  const isBase64 = headerParts.includes("base64");

  try {
    const bytes = isBase64
      ? Uint8Array.from(window.atob(payload), (character) => character.charCodeAt(0))
      : new TextEncoder().encode(decodeURIComponent(payload));

    return new Blob([bytes], { type: mimeType });
  } catch {
    throw new Error("Saved source audio could not be loaded.");
  }
}

export async function convertBlobToWav(blob: Blob) {
  if (blob.type.includes("wav")) return blob;

  const AudioContextCtor = window.AudioContext || (window as Window & { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
  if (!AudioContextCtor) return blob;

  const audioContext = new AudioContextCtor();
  if (typeof audioContext.decodeAudioData !== "function") {
    await audioContext.close?.().catch(() => {});
    return blob;
  }

  const sourceData = await blob.arrayBuffer();
  const decoded = await audioContext.decodeAudioData(sourceData.slice(0));
  await audioContext.close?.().catch(() => {});
  const monoSamples = mixAudioBufferToMono(decoded);
  const samples = resamplePcm(monoSamples, decoded.sampleRate, 16000);
  const wavData = encodePcm16Wav(samples, 16000);

  return new Blob([wavData], { type: "audio/wav" });
}

export function mixAudioBufferToMono(audioBuffer: AudioBuffer) {
  const samples = new Float32Array(audioBuffer.length);
  const channelCount = Math.max(1, audioBuffer.numberOfChannels);

  for (let channel = 0; channel < channelCount; channel += 1) {
    const channelData = audioBuffer.getChannelData(channel);
    for (let index = 0; index < samples.length; index += 1) {
      samples[index] += channelData[index] / channelCount;
    }
  }

  return samples;
}

export function resamplePcm(samples: Float32Array, sourceRate: number, targetRate: number) {
  if (sourceRate === targetRate) return samples;

  const targetLength = Math.max(1, Math.round(samples.length * targetRate / sourceRate));
  const resampled = new Float32Array(targetLength);
  const ratio = (samples.length - 1) / Math.max(1, targetLength - 1);

  for (let index = 0; index < targetLength; index += 1) {
    const sourceIndex = index * ratio;
    const lower = Math.floor(sourceIndex);
    const upper = Math.min(samples.length - 1, lower + 1);
    const weight = sourceIndex - lower;
    resampled[index] = samples[lower] * (1 - weight) + samples[upper] * weight;
  }

  return resampled;
}

export function encodePcm16Wav(samples: Float32Array, sampleRate: number) {
  const bytesPerSample = 2;
  const dataLength = samples.length * bytesPerSample;
  const buffer = new ArrayBuffer(44 + dataLength);
  const view = new DataView(buffer);

  writeAscii(view, 0, "RIFF");
  view.setUint32(4, 36 + dataLength, true);
  writeAscii(view, 8, "WAVE");
  writeAscii(view, 12, "fmt ");
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, 1, true);
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, sampleRate * bytesPerSample, true);
  view.setUint16(32, bytesPerSample, true);
  view.setUint16(34, 8 * bytesPerSample, true);
  writeAscii(view, 36, "data");
  view.setUint32(40, dataLength, true);

  let offset = 44;
  for (const sample of samples) {
    const clamped = Math.max(-1, Math.min(1, sample));
    view.setInt16(offset, Math.round(clamped < 0 ? clamped * 32768 : clamped * 32767), true);
    offset += bytesPerSample;
  }

  return buffer;
}

function writeAscii(view: DataView, offset: number, value: string) {
  for (let index = 0; index < value.length; index += 1) {
    view.setUint8(offset + index, value.charCodeAt(index));
  }
}

export function readBlobAsDataUrl(blob: Blob) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error("Failed to save recording audio"));
    reader.onload = () => {
      if (typeof reader.result === "string") {
        resolve(reader.result);
        return;
      }

      reject(new Error("Failed to save recording audio"));
    };
    reader.readAsDataURL(blob);
  });
}
