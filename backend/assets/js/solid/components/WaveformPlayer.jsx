import { createSignal, createEffect, Show, untrack } from "solid-js";

// Module-level cache for decoded AudioBuffers (survives component re-renders)
const audioBufferCache = new Map();

export function WaveformPlayer(props) {
  // props: audioUrl, cacheKey (optional), currentTime (optional), isPlaying (optional), onTimeUpdate, onPlayingChange

  let canvasRef;
  let audioRef;
  const [localPlaying, setLocalPlaying] = createSignal(false);
  const [localTime, setLocalTime] = createSignal(0);
  const [duration, setDuration] = createSignal(0);
  const [audioBuffer, setAudioBuffer] = createSignal(null);
  const [loading, setLoading] = createSignal(true);

  // Use props if provided, otherwise local state
  const isPlaying = () => props.isPlaying !== undefined ? props.isPlaying : localPlaying();
  const currentTime = () => props.currentTime !== undefined ? props.currentTime : localTime();

  // Track if we need to restore position/play state after load
  let pendingSeek = null;
  let pendingPlay = false;
  let lastUrl = null;

  // Load audio when URL changes (only track URL, not other props)
  createEffect(async () => {
    const url = props.audioUrl;
    if (!url || url === lastUrl) return;

    // Capture current state without creating dependencies
    pendingSeek = untrack(() => currentTime());
    pendingPlay = untrack(() => isPlaying());
    lastUrl = url;

    // Check cache first - use cacheKey if provided, otherwise URL
    const key = props.cacheKey || url;
    const cached = audioBufferCache.get(key);
    if (cached) {
      setAudioBuffer(cached);
      setDuration(cached.duration);
      setLoading(false);
      drawWaveform(cached);
      return;
    }

    setLoading(true);
    setAudioBuffer(null);

    try {
      const response = await fetch(url);
      const arrayBuffer = await response.arrayBuffer();
      const audioContext = new AudioContext();
      const buffer = await audioContext.decodeAudioData(arrayBuffer);

      // Cache the decoded buffer using cacheKey if provided
      audioBufferCache.set(key, buffer);

      setAudioBuffer(buffer);
      setDuration(buffer.duration);
      setLoading(false);
      drawWaveform(buffer);
    } catch (err) {
      console.error("Failed to decode audio:", err);
      setLoading(false);
    }
  });

  // Handle audio ready - restore position and play state
  function handleCanPlay() {
    if (pendingSeek !== null && pendingSeek > 0) {
      audioRef.currentTime = pendingSeek;
      pendingSeek = null;
    }
    if (pendingPlay) {
      audioRef.play();
      pendingPlay = false;
    }
  }

  function drawWaveform(buffer) {
    if (!canvasRef || !buffer) return;

    const ctx = canvasRef.getContext("2d");
    const data = buffer.getChannelData(0);
    const width = canvasRef.width;
    const height = canvasRef.height;
    const step = Math.ceil(data.length / width);

    ctx.clearRect(0, 0, width, height);
    ctx.fillStyle = "#7c3aed";

    for (let i = 0; i < width; i++) {
      const start = i * step;
      let min = 1, max = -1;
      for (let j = 0; j < step; j++) {
        const val = data[start + j] || 0;
        if (val < min) min = val;
        if (val > max) max = val;
      }
      const y1 = ((1 + min) / 2) * height;
      const y2 = ((1 + max) / 2) * height;
      ctx.fillRect(i, y1, 1, Math.max(1, y2 - y1));
    }
  }

  function drawPlayhead() {
    const buffer = audioBuffer();
    if (!canvasRef || !buffer) return;

    drawWaveform(buffer);

    const ctx = canvasRef.getContext("2d");
    const x = (currentTime() / duration()) * canvasRef.width;
    ctx.fillStyle = "#ffffff";
    ctx.fillRect(x - 1, 0, 2, canvasRef.height);
  }

  createEffect(() => {
    currentTime();
    drawPlayhead();
  });

  function togglePlay() {
    const newPlaying = !isPlaying();

    if (newPlaying) {
      audioRef.play();
    } else {
      audioRef.pause();
    }

    // Notify parent or update local
    if (props.onPlayingChange) {
      props.onPlayingChange(newPlaying);
    } else {
      setLocalPlaying(newPlaying);
    }
  }

  function handleTimeUpdate() {
    const time = audioRef.currentTime;

    // Notify parent or update local
    if (props.onTimeUpdate) {
      props.onTimeUpdate(time);
    } else {
      setLocalTime(time);
    }
  }

  function handleCanvasClick(e) {
    const rect = canvasRef.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const percent = x / canvasRef.width;
    const newTime = percent * duration();

    audioRef.currentTime = newTime;

    if (props.onTimeUpdate) {
      props.onTimeUpdate(newTime);
    } else {
      setLocalTime(newTime);
    }
  }

  function handleEnded() {
    if (props.onPlayingChange) {
      props.onPlayingChange(false);
    } else {
      setLocalPlaying(false);
    }
  }

  function formatTime(seconds) {
    const m = Math.floor(seconds / 60);
    const s = Math.floor(seconds % 60);
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  return (
    <div class="space-y-3">
      <audio
        ref={el => audioRef = el}
        src={props.audioUrl}
        onTimeUpdate={handleTimeUpdate}
        onEnded={handleEnded}
        onLoadedMetadata={() => setDuration(audioRef.duration)}
        onCanPlay={handleCanPlay}
      />

      <Show when={loading()}>
        <div class="w-full h-20 rounded-lg bg-base-300 flex items-center justify-center">
          <span class="loading loading-spinner loading-sm" />
        </div>
      </Show>

      <Show when={!loading()}>
        <canvas
          ref={el => canvasRef = el}
          width={400}
          height={80}
          class="w-full h-20 rounded-lg bg-base-300 cursor-pointer"
          onClick={handleCanvasClick}
        />
      </Show>

      <div class="flex items-center gap-4">
        <button
          type="button"
          onClick={togglePlay}
          disabled={loading()}
          class="btn btn-circle btn-sm btn-primary"
        >
          <Show when={isPlaying()} fallback={
            <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 24 24">
              <path d="M8 5v14l11-7z"/>
            </svg>
          }>
            <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 24 24">
              <path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z"/>
            </svg>
          </Show>
        </button>
        <span class="text-sm font-mono text-base-content/70">
          {formatTime(currentTime())} / {formatTime(duration())}
        </span>
      </div>
    </div>
  );
}
