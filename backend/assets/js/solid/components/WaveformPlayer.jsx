import { createSignal, createEffect, Show, untrack, onCleanup } from "solid-js";

// Module-level cache for decoded AudioBuffers with LRU eviction
const MAX_CACHE_ENTRIES = 5;
const audioBufferCache = new Map();

function cacheBuffer(key, buffer) {
  // Evict oldest entry if at limit (Map maintains insertion order)
  if (audioBufferCache.size >= MAX_CACHE_ENTRIES) {
    const firstKey = audioBufferCache.keys().next().value;
    audioBufferCache.delete(firstKey);
  }
  audioBufferCache.set(key, buffer);
}

// Clear all cached buffers (call on reset/new job)
export function clearAudioCache() {
  audioBufferCache.clear();
}

// Shared AudioContext to avoid browser throttling (limit ~6-10 concurrent)
let sharedAudioContext = null;

function getAudioContext() {
  if (!sharedAudioContext || sharedAudioContext.state === "closed") {
    sharedAudioContext = new AudioContext();
  }
  return sharedAudioContext;
}

export function WaveformPlayer(props) {
  // props: audioUrl, cacheKey (optional), currentTime (optional), isPlaying (optional), onTimeUpdate, onPlayingChange

  let canvasRef;
  let audioRef;
  const [localPlaying, setLocalPlaying] = createSignal(false);
  const [localTime, setLocalTime] = createSignal(0);
  const [duration, setDuration] = createSignal(0);
  const [audioBuffer, setAudioBuffer] = createSignal(null);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal(null);

  // Use props if provided, otherwise local state
  const isPlaying = () => props.isPlaying !== undefined ? props.isPlaying : localPlaying();
  const currentTime = () => props.currentTime !== undefined ? props.currentTime : localTime();

  // Track if we need to restore position/play state after load
  let pendingSeek = null;
  let pendingPlay = false;
  let lastUrl = null;
  let abortController = null;

  // Cleanup: abort any in-flight fetch when component unmounts
  onCleanup(() => {
    if (abortController) {
      abortController.abort();
    }
  });

  // Load audio from URL
  async function loadAudio(url) {
    if (!url) return;

    // Cancel any in-progress fetch to prevent race conditions
    if (abortController) {
      abortController.abort();
    }
    abortController = new AbortController();
    const currentAbortController = abortController;

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
      setError(null);
      // For cached audio, seek immediately after a microtask (DOM update)
      queueMicrotask(() => {
        if (audioRef && pendingSeek !== null) {
          audioRef.currentTime = pendingSeek;
          pendingSeek = null;
        }
        if (audioRef && pendingPlay) {
          audioRef.play();
          pendingPlay = false;
        }
      });
      return;
    }

    setLoading(true);
    setAudioBuffer(null);
    setError(null);

    try {
      const response = await fetch(url, { signal: currentAbortController.signal });
      if (!response.ok) {
        throw new Error(`Failed to fetch audio: ${response.status}`);
      }
      const arrayBuffer = await response.arrayBuffer();

      // Check if this fetch was aborted while reading body
      if (currentAbortController.signal.aborted) {
        return;
      }

      const audioContext = getAudioContext();
      const buffer = await audioContext.decodeAudioData(arrayBuffer);

      // Check again after decode (another async operation)
      if (currentAbortController.signal.aborted) {
        return;
      }

      // Cache the decoded buffer using cacheKey if provided (LRU eviction)
      cacheBuffer(key, buffer);

      setAudioBuffer(buffer);
      setDuration(buffer.duration);
      setLoading(false);
    } catch (err) {
      // Ignore abort errors - they're intentional
      if (err.name === "AbortError") {
        return;
      }
      console.error("Failed to decode audio:", err);
      setLoading(false);
      setError("Unable to play audio");
    }
  }

  function retryLoad() {
    const url = props.audioUrl;
    if (url) {
      lastUrl = null; // Reset to allow reload
      loadAudio(url);
    }
  }

  // Load audio when URL changes (only track URL, not other props)
  createEffect(() => {
    const url = props.audioUrl;
    if (!url || url === lastUrl) return;
    loadAudio(url);
  });

  // Handle audio ready - restore position and play state after URL change
  function handleCanPlay() {
    if (pendingSeek !== null) {
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

  // Draw waveform when buffer is ready AND canvas is mounted
  // (canvas only mounts after loading() becomes false)
  createEffect(() => {
    const buffer = audioBuffer();
    if (!loading() && buffer) {
      drawWaveform(buffer);
    }
  });

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
    // Use displayed width (rect.width), not canvas internal width (canvasRef.width)
    const percent = x / rect.width;
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

      <Show when={error()}>
        <div class="w-full h-20 rounded-lg bg-base-300 flex flex-col items-center justify-center gap-2">
          <span class="text-sm text-error">{error()}</span>
          <button
            type="button"
            onClick={retryLoad}
            class="btn btn-xs btn-ghost"
          >
            Try again
          </button>
        </div>
      </Show>

      <Show when={!loading() && !error()}>
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
          disabled={loading() || error()}
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
