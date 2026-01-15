# Trial Feature: 10-Second Preview Comparison

## Goal
Let users compare 10-second clips with different presets to find the best settings before committing to full processing.

**Business context:** All processing is paid (including trials), but processing 10 seconds is much cheaper than a full file. Users can experiment with different presets/sections at low cost before committing to expensive full-file processing.

---

## User Flow

```
1. User uploads audio file (MP3, WAV, FLAC, etc.)
       ↓
2. Phoenix sends to Rust /convert endpoint
       ↓
3. Rust decodes → WAV → uploads to S3
       ↓
4. Browser shows waveform, user can play/seek original
       ↓
5. User positions playhead, selects preset, clicks "Try 10s"
       ↓
6. Rust pulls 10s from S3 (byte-range), processes, returns Opus
       ↓
7. Browser plays processed clip, user can A/B compare with original
       ↓
8. User tries different presets/sections until satisfied
       ↓
9. User clicks "MUNCH IT" → full processing
```

---

## Technical Architecture

### Phase 1: Upload → WAV Conversion → S3

**Why WAV in S3:**
- Fixed-size frames = exact byte positions for any time
- S3 byte-range requests work perfectly
- No frame-boundary issues like MP3/FLAC

**New Rust endpoint: `POST /convert`**

Request (multipart):
```
Content-Type: multipart/form-data
- file: audio file bytes
- filename: original filename
- user_id: user identifier
```

Response:
```json
{
  "s3_key": "inputs/user123/abc123.wav",
  "duration": 245.5,
  "sample_rate": 44100,
  "channels": 2
}
```

**Implementation (`src/handlers/convert.rs`):**
1. Receive multipart audio file
2. Decode with symphonia (any format → PCM samples)
3. Encode to WAV with hound (16-bit PCM)
4. Upload WAV to S3
5. Return metadata

### Phase 2: Trial Processing

**New Rust endpoint: `POST /trial`**

Request:
```json
{
  "s3_key": "inputs/user123/abc123.wav",
  "start_time": 30.0,
  "duration": 10.0,
  "chain": "podcast",
  "sample_rate": 44100,
  "channels": 2
}
```

Response:
```
Content-Type: audio/ogg
Body: Opus-encoded audio (~160KB for 10s @ 128kbps)
```

**Byte-range calculation:**
```rust
const WAV_HEADER_SIZE: u64 = 44;
let bytes_per_sample: u64 = 2;  // 16-bit
let bytes_per_frame: u64 = channels as u64 * bytes_per_sample;

let start_byte = WAV_HEADER_SIZE + (start_time * sample_rate as f32 * bytes_per_frame as f32) as u64;
let end_byte = WAV_HEADER_SIZE + ((start_time + duration) * sample_rate as f32 * bytes_per_frame as f32) as u64;
```

**Implementation (`src/handlers/trial.rs`):**
1. Calculate byte range from time position
2. Download range from S3 using `get_object_range()`
3. Wrap raw PCM in minimal WAV header for processing
4. Run through processing chain (same as full job, but on 10s)
5. Encode result to Opus @ 128kbps
6. Return audio bytes

### Phase 3: Browser Playback & A/B Comparison

**Original audio:** Already loaded in AudioBuffer from waveform visualization

**Processed audio:**
1. Receive Opus blob from trial endpoint
2. Decode with `audioContext.decodeAudioData()`
3. Cache in memory for instant replay

**A/B Toggle:**
- Both original and processed are AudioBuffers
- Toggle switches which one plays
- Playback position stays synced

---

## UI Design

### Trial Button
Near playback controls, visible when file is uploaded:

```
[⏹] [▶]  0:30 / 3:45    [🎧 Try 10s]
```

### After Trial Completes
Show A/B comparison controls:

```
┌────────────────────────────────────────────────┐
│  ╭──────────────────────────────────────────╮  │
│  │▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓░░░░░░░░░░░░░░░░░░░░░░░░░░│  │  ← Waveform
│  ╰──────────────────────────────────────────╯  │
│                                                │
│  [⏹] [▶]  0:30 / 3:45                         │
│                                                │
│  ┌──────────────────────────────────────────┐  │
│  │  Trial: 0:30 - 0:40  (Podcast preset)    │  │
│  │                                          │  │
│  │  [▶ A: Original]    [▶ B: Processed]     │  │
│  │       ↑ playing                          │  │
│  │                                          │  │
│  │  [Try different section]  [Try again]    │  │
│  └──────────────────────────────────────────┘  │
│                                                │
│  Preset: [Podcast ▾]                          │
│                                                │
│  [🐄 MUNCH IT!]                               │
└────────────────────────────────────────────────┘
```

### State Management
- Cache last 3 trial results in browser memory
- Each trial: `{ preset, startTime, originalBuffer, processedBuffer }`
- Clear cache on page refresh or new file upload

---

## Files to Create/Modify

### Rust API

**`src/handlers/convert.rs`** (new)
```rust
use axum::{extract::Multipart, Json};
use symphonia::core::io::MediaSourceStream;
use hound::{WavWriter, WavSpec};

pub async fn convert_audio(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<ConvertResponse>, ApiError> {
    // 1. Extract file from multipart
    // 2. Decode with symphonia
    // 3. Encode to WAV with hound
    // 4. Upload to S3
    // 5. Return metadata
}
```

**`src/handlers/trial.rs`** (new)
```rust
use audiopus::{Encoder, SampleRate, Channels, Application};
use ogg::PacketWriter;

pub async fn process_trial(
    State(state): State<AppState>,
    Json(req): Json<TrialRequest>,
) -> Result<Response<Body>, ApiError> {
    // 1. Calculate byte range
    // 2. Download range from S3
    // 3. Process audio
    // 4. Encode to Opus
    // 5. Return audio/ogg response
}
```

**`src/storage.rs`** - add method:
```rust
pub async fn download_range(&self, key: &str, start: u64, end: u64) -> Result<Vec<u8>> {
    let response = self.bucket
        .get_object_range(key, start, Some(end))
        .await
        .context("Failed to download range from S3")?;
    Ok(response.bytes().to_vec())
}
```

**`src/audio/opus.rs`** (new)
```rust
pub fn encode_opus(samples: &[f32], sample_rate: u32, channels: u8) -> Result<Vec<u8>> {
    // Encode to Opus @ 128kbps
    // Wrap in Ogg container
}
```

**`src/main.rs`** - add routes:
```rust
.route("/convert", post(handlers::convert::convert_audio))
.route("/trial", post(handlers::trial::process_trial))
```

**`Cargo.toml`** - add dependencies:
```toml
audiopus = "0.3"
ogg = "0.9"
```

### Phoenix

**`lib/poddyclip_backend/processing.ex`** - add functions:
```elixir
def convert_file(file_path, user_id) do
  # POST to Rust /convert with multipart
  # Return {:ok, %{s3_key, duration, sample_rate, channels}}
end

def request_trial(s3_key, start_time, preset, metadata) do
  # POST to Rust /trial
  # Return {:ok, opus_binary}
end
```

**`lib/poddyclip_backend_web/live/process_live.ex`**:
- Change upload flow to use `/convert` endpoint
- Add trial-related assigns: `trial_results`, `current_trial`, `comparing`
- Add event handlers: `"request_trial"`, `"play_original"`, `"play_processed"`
- Add trial UI components

### JavaScript

**`assets/js/app.js`** - extend AudioPreview hook:
```javascript
const AudioPreview = {
  // ... existing code ...

  // New: Trial functionality
  trials: [],  // Cache of trial results

  async requestTrial(preset) {
    const startTime = this.audio.currentTime;
    // Send trial request via LiveView
    this.pushEvent("request_trial", {
      start_time: startTime,
      preset: preset
    });
  },

  receiveTrialResult(opusData, preset, startTime) {
    // Decode Opus and cache
    const arrayBuffer = Uint8Array.from(atob(opusData), c => c.charCodeAt(0)).buffer;
    this.audioContext.decodeAudioData(arrayBuffer, (buffer) => {
      this.trials.push({ preset, startTime, buffer });
      this.showComparison(startTime, buffer);
    });
  },

  playOriginal(startTime, duration) {
    // Play original from main audio element
    this.audio.currentTime = startTime;
    this.audio.play();
    setTimeout(() => this.audio.pause(), duration * 1000);
  },

  playProcessed(trialIndex) {
    // Play from cached AudioBuffer
    const source = this.audioContext.createBufferSource();
    source.buffer = this.trials[trialIndex].buffer;
    source.connect(this.audioContext.destination);
    source.start();
  }
}
```

---

## Implementation Order

### Phase 1: Rust /convert endpoint
1. Add `audiopus` and `ogg` to Cargo.toml
2. Create `src/handlers/convert.rs`
3. Add route to main.rs
4. Test: upload MP3, verify WAV in S3

### Phase 2: Rust /trial endpoint
1. Add `download_range()` to storage.rs
2. Create `src/audio/opus.rs` for Opus encoding
3. Create `src/handlers/trial.rs`
4. Add route to main.rs
5. Test: request trial, verify Opus response

### Phase 3: Phoenix integration
1. Update processing.ex with convert/trial functions
2. Change upload flow in process_live.ex
3. Add trial state and event handlers
4. Test: upload → convert → trial request works

### Phase 4: Browser UI
1. Add trial button to playback controls
2. Add A/B comparison panel
3. Implement AudioPreview trial methods
4. Cache management
5. Test: full user flow works

### Phase 5: Polish
1. Loading states during trial processing
2. Error handling (trial fails, network issues)
3. Keyboard shortcuts (A/B toggle with spacebar?)
4. Visual feedback for which version is playing

---

## Verification Checklist

1. **Upload conversion:**
   - [ ] Upload MP3 → WAV stored in S3
   - [ ] Upload FLAC → WAV stored in S3
   - [ ] Metadata (duration, sample_rate, channels) correct

2. **Trial processing:**
   - [ ] Request trial at 0:30 → correct 10s extracted
   - [ ] Processing applied (audible difference)
   - [ ] Opus response plays in browser
   - [ ] Response size ~160KB

3. **A/B comparison:**
   - [ ] Original plays correctly
   - [ ] Processed plays correctly
   - [ ] Can toggle between them
   - [ ] Position stays synced

4. **Multiple trials:**
   - [ ] Try different presets on same section
   - [ ] Try same preset on different sections
   - [ ] Cache works (instant replay of previous trials)

5. **Full flow:**
   - [ ] Trial → satisfied → MUNCH IT → uses correct preset
   - [ ] Full-file processing works as before

---

## Future Enhancements (not in scope)

- Compare two different presets side-by-side (A vs B vs C)
- Save favorite trial results
- Share trial clips
- Waveform visualization of processed audio
