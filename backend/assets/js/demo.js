/**
 * Demo widget for anonymous audio processing.
 * Self-contained vanilla JS, dynamically imported from app.js.
 */

const STATES = { IDLE: 0, VALIDATING: 1, UPLOADING: 2, PROCESSING: 3, COMPLETE: 4, ERROR: 5 }
const MAX_DURATION = 30
const MAX_SIZE = 10 * 1024 * 1024
const POLL_INTERVAL = 2000

export function initDemo(container) {
  const csrfToken = document.querySelector("meta[name='csrf-token']")?.getAttribute("content")
  let state = STATES.IDLE
  let jobId = null
  let pollTimer = null

  // Build UI
  container.innerHTML = `
    <div id="demo-inner" class="max-w-md mx-auto">
      <div id="demo-drop" class="border-2 border-dashed border-base-300 rounded-xl p-10 text-center cursor-pointer hover:border-primary transition-colors">
        <p class="text-lg font-medium mb-2">Drop an audio file here</p>
        <p class="text-sm text-base-content/60">or click to browse. Max 30 seconds, 10MB.</p>
        <input type="file" id="demo-file" accept="audio/*" class="hidden" />
      </div>
      <div id="demo-progress" class="hidden mt-4">
        <div class="flex justify-between text-sm mb-1">
          <span id="demo-stage">Preparing...</span>
          <span id="demo-percent"></span>
        </div>
        <progress id="demo-bar" class="progress progress-primary w-full" value="0" max="100"></progress>
      </div>
      <div id="demo-error" class="hidden mt-4 alert alert-error text-sm"></div>
      <div id="demo-result" class="hidden mt-6">
        <div class="flex gap-2 mb-3">
          <button id="demo-toggle-original" class="btn btn-sm btn-outline btn-active">Original</button>
          <button id="demo-toggle-processed" class="btn btn-sm btn-outline">Processed</button>
        </div>
        <audio id="demo-audio" controls class="w-full"></audio>
        <div class="mt-4 text-center">
          <a href="/users/log-in" class="btn btn-primary">Like what you hear? Sign up free</a>
        </div>
      </div>
    </div>
  `

  const dropZone = container.querySelector("#demo-drop")
  const fileInput = container.querySelector("#demo-file")
  const progressDiv = container.querySelector("#demo-progress")
  const stageEl = container.querySelector("#demo-stage")
  const percentEl = container.querySelector("#demo-percent")
  const barEl = container.querySelector("#demo-bar")
  const errorDiv = container.querySelector("#demo-error")
  const resultDiv = container.querySelector("#demo-result")
  const audioEl = container.querySelector("#demo-audio")
  const btnOriginal = container.querySelector("#demo-toggle-original")
  const btnProcessed = container.querySelector("#demo-toggle-processed")

  let originalUrl = null
  let processedUrl = null

  // Events
  dropZone.addEventListener("click", () => fileInput.click())
  dropZone.addEventListener("dragover", e => { e.preventDefault(); dropZone.classList.add("border-primary") })
  dropZone.addEventListener("dragleave", () => dropZone.classList.remove("border-primary"))
  dropZone.addEventListener("drop", e => {
    e.preventDefault()
    dropZone.classList.remove("border-primary")
    if (e.dataTransfer.files.length) handleFile(e.dataTransfer.files[0])
  })
  fileInput.addEventListener("change", () => {
    if (fileInput.files.length) handleFile(fileInput.files[0])
  })

  btnOriginal.addEventListener("click", () => switchAudio("original"))
  btnProcessed.addEventListener("click", () => switchAudio("processed"))

  function switchAudio(which) {
    const currentTime = audioEl.currentTime
    const wasPlaying = !audioEl.paused
    audioEl.src = which === "original" ? originalUrl : processedUrl
    audioEl.currentTime = currentTime
    if (wasPlaying) audioEl.play()
    btnOriginal.classList.toggle("btn-active", which === "original")
    btnProcessed.classList.toggle("btn-active", which === "processed")
  }

  function showError(msg) {
    state = STATES.ERROR
    errorDiv.textContent = msg
    errorDiv.classList.remove("hidden")
    progressDiv.classList.add("hidden")
    dropZone.classList.remove("hidden")
  }

  function hideError() {
    errorDiv.classList.add("hidden")
  }

  async function handleFile(file) {
    if (state !== STATES.IDLE && state !== STATES.ERROR && state !== STATES.COMPLETE) return
    hideError()
    resultDiv.classList.add("hidden")

    // Size check
    if (file.size > MAX_SIZE) {
      return showError("File too large. Maximum 10MB.")
    }

    // Duration check via Web Audio API
    state = STATES.VALIDATING
    stageEl.textContent = "Checking duration..."
    percentEl.textContent = ""
    barEl.value = 0
    progressDiv.classList.remove("hidden")
    dropZone.classList.add("hidden")

    try {
      const duration = await getAudioDuration(file)
      if (duration > MAX_DURATION) {
        dropZone.classList.remove("hidden")
        return showError(`Audio is ${Math.round(duration)}s. Maximum is ${MAX_DURATION} seconds.`)
      }
    } catch {
      dropZone.classList.remove("hidden")
      return showError("Could not read audio file. Try a different format.")
    }

    // Get presigned URL
    state = STATES.UPLOADING
    stageEl.textContent = "Uploading..."

    try {
      const presignRes = await fetch("/api/demo/presign-upload", {
        method: "POST",
        headers: { "content-type": "application/json", "x-csrf-token": csrfToken },
        body: JSON.stringify({ filename: file.name })
      })

      if (presignRes.status === 429) {
        dropZone.classList.remove("hidden")
        return showError("Demo limit reached (3 per day). Sign up for free to process more.")
      }

      if (!presignRes.ok) {
        dropZone.classList.remove("hidden")
        const err = await presignRes.json().catch(() => ({}))
        return showError(err.error || "Upload failed. Please try again.")
      }

      const { url: uploadUrl, key: s3Key } = await presignRes.json()

      // Upload to S3 with progress
      await uploadToS3(uploadUrl, file, pct => {
        barEl.value = pct
        percentEl.textContent = `${pct}%`
      })

      // Start processing
      state = STATES.PROCESSING
      stageEl.textContent = "Processing..."
      barEl.value = 0
      percentEl.textContent = ""

      const processRes = await fetch("/api/demo/process", {
        method: "POST",
        headers: { "content-type": "application/json", "x-csrf-token": csrfToken },
        body: JSON.stringify({ s3_key: s3Key, filename: file.name })
      })

      if (processRes.status === 429) {
        dropZone.classList.remove("hidden")
        return showError("Demo limit reached (3 per day). Sign up for free to process more.")
      }

      if (!processRes.ok) {
        dropZone.classList.remove("hidden")
        return showError("Processing failed. Please try again.")
      }

      const { id } = await processRes.json()
      jobId = id
      startPolling()

    } catch (err) {
      dropZone.classList.remove("hidden")
      showError("Something went wrong. Please try again.")
    }
  }

  function startPolling() {
    pollTimer = setInterval(async () => {
      try {
        const res = await fetch(`/api/demo/jobs/${jobId}/status`)
        if (!res.ok) { clearInterval(pollTimer); return showError("Lost connection to server.") }

        const data = await res.json()

        if (data.status === "processing" || data.status === "queued") {
          const progress = data.progress || {}
          stageEl.textContent = "Processing..."
          if (progress.percent_complete != null) {
            barEl.value = progress.percent_complete
            percentEl.textContent = `${progress.percent_complete}%`
          }
        } else if (data.status === "completed") {
          clearInterval(pollTimer)
          state = STATES.COMPLETE
          progressDiv.classList.add("hidden")

          originalUrl = data.original_url
          processedUrl = data.download_url

          audioEl.src = processedUrl
          resultDiv.classList.remove("hidden")
          btnOriginal.classList.remove("btn-active")
          btnProcessed.classList.add("btn-active")
          dropZone.classList.remove("hidden")
        } else if (data.status === "failed") {
          clearInterval(pollTimer)
          dropZone.classList.remove("hidden")
          showError(data.error || "Processing failed.")
        }
      } catch {
        clearInterval(pollTimer)
        dropZone.classList.remove("hidden")
        showError("Lost connection. Please try again.")
      }
    }, POLL_INTERVAL)
  }

  function getAudioDuration(file) {
    return new Promise((resolve, reject) => {
      const audio = new Audio()
      audio.preload = "metadata"
      audio.onloadedmetadata = () => {
        URL.revokeObjectURL(audio.src)
        resolve(audio.duration)
      }
      audio.onerror = () => {
        URL.revokeObjectURL(audio.src)
        reject(new Error("Cannot read audio"))
      }
      audio.src = URL.createObjectURL(file)
    })
  }

  function uploadToS3(url, file, onProgress) {
    return new Promise((resolve, reject) => {
      const xhr = new XMLHttpRequest()
      xhr.open("PUT", url)
      xhr.setRequestHeader("Content-Type", "application/octet-stream")
      xhr.upload.onprogress = e => {
        if (e.lengthComputable) onProgress(Math.round((e.loaded / e.total) * 100))
      }
      xhr.onload = () => xhr.status >= 200 && xhr.status < 300 ? resolve() : reject(new Error(`Upload failed: ${xhr.status}`))
      xhr.onerror = () => reject(new Error("Upload failed"))
      xhr.send(file)
    })
  }
}
