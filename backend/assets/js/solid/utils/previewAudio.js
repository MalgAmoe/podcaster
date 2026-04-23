const DEFAULT_PREVIEW_SECONDS = 20;

function getAudioContextCtor() {
  return window.AudioContext || window.webkitAudioContext;
}

function clampSample(sample) {
  return Math.max(-1, Math.min(1, sample));
}

function writeAscii(view, offset, value) {
  for (let i = 0; i < value.length; i += 1) {
    view.setUint8(offset + i, value.charCodeAt(i));
  }
}

function encodeWav({ channelData, sampleRate }) {
  const channelCount = channelData.length;
  const frameCount = channelData[0]?.length || 0;
  const bytesPerSample = 2;
  const blockAlign = channelCount * bytesPerSample;
  const byteRate = sampleRate * blockAlign;
  const dataSize = frameCount * blockAlign;
  const buffer = new ArrayBuffer(44 + dataSize);
  const view = new DataView(buffer);

  writeAscii(view, 0, "RIFF");
  view.setUint32(4, 36 + dataSize, true);
  writeAscii(view, 8, "WAVE");
  writeAscii(view, 12, "fmt ");
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, channelCount, true);
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, byteRate, true);
  view.setUint16(32, blockAlign, true);
  view.setUint16(34, 16, true);
  writeAscii(view, 36, "data");
  view.setUint32(40, dataSize, true);

  let offset = 44;
  for (let frame = 0; frame < frameCount; frame += 1) {
    for (let channel = 0; channel < channelCount; channel += 1) {
      const sample = clampSample(channelData[channel][frame]);
      const int16 = sample < 0 ? sample * 0x8000 : sample * 0x7fff;
      view.setInt16(offset, int16, true);
      offset += bytesPerSample;
    }
  }

  return buffer;
}

function getPreviewFilename(filename) {
  const dotIndex = filename.lastIndexOf(".");
  const baseName = dotIndex > 0 ? filename.slice(0, dotIndex) : filename;
  return `${baseName}-preview.wav`;
}

export async function trimAudioFileForPreview(file, maxSeconds = DEFAULT_PREVIEW_SECONDS) {
  const AudioContextCtor = getAudioContextCtor();
  if (!AudioContextCtor) {
    throw new Error("This browser cannot prepare audio previews.");
  }

  const audioContext = new AudioContextCtor();

  try {
    const arrayBuffer = await file.arrayBuffer();
    const audioBuffer = await audioContext.decodeAudioData(arrayBuffer.slice(0));
    const maxFrames = Math.max(1, Math.ceil(maxSeconds * audioBuffer.sampleRate));
    const trimmedFrames = Math.min(audioBuffer.length, maxFrames);
    const channelData = [];

    for (let channel = 0; channel < audioBuffer.numberOfChannels; channel += 1) {
      channelData.push(audioBuffer.getChannelData(channel).slice(0, trimmedFrames));
    }

    const wavBuffer = encodeWav({
      channelData,
      sampleRate: audioBuffer.sampleRate,
    });

    const clippedSeconds = Math.ceil(trimmedFrames / audioBuffer.sampleRate);
    const originalSeconds = Math.ceil(audioBuffer.duration);
    const previewFile = new File(
      [wavBuffer],
      getPreviewFilename(file.name),
      {
        type: "audio/wav",
        lastModified: file.lastModified,
      },
    );

    return {
      file: previewFile,
      originalFilename: file.name,
      originalSeconds,
      clippedSeconds,
      wasTrimmed: trimmedFrames < audioBuffer.length,
    };
  } finally {
    await audioContext.close().catch(() => {});
  }
}
