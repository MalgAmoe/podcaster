// If you want to use Phoenix channels, run `mix help phx.gen.channel`
// to get started and then uncomment the line below.
// import "./user_socket.js"

// You can include dependencies in two ways.
//
// The simplest option is to put them in assets/vendor and
// import them using relative paths:
//
//     import "../vendor/some-package.js"
//
// Alternatively, you can `npm install some-package --prefix assets` and import
// them using a path starting with the package name:
//
//     import "some-package"
//
// If you have dependencies that try to import CSS, esbuild will generate a separate `app.css` file.
// To load it, simply add a second `<link>` to your `root.html.heex` file.

// Include phoenix_html to handle method=PUT/DELETE in forms and buttons.
import "phoenix_html"
// Establish Phoenix Socket and LiveView configuration.
import {Socket} from "phoenix"
import {LiveSocket} from "phoenix_live_view"
import {hooks as colocatedHooks} from "phoenix-colocated/poddyclip_backend"
import topbar from "../vendor/topbar"

const csrfToken = document.querySelector("meta[name='csrf-token']").getAttribute("content")

// Store uploaded files for audio preview
window.uploadedFiles = {}

// S3 direct upload handler for LiveView external uploads
const Uploaders = {
  S3(entries, onViewError) {
    entries.forEach(entry => {
      const { url } = entry.meta

      // Store file for audio preview hook
      window.uploadedFiles[entry.ref] = entry.file

      const xhr = new XMLHttpRequest()

      // Track upload progress
      xhr.upload.addEventListener("progress", (event) => {
        if (event.lengthComputable) {
          const percent = Math.round((event.loaded / event.total) * 100)
          entry.progress(percent)
        }
      })

      xhr.addEventListener("load", () => {
        if (xhr.status >= 200 && xhr.status < 300) {
          entry.progress(100)
        } else {
          entry.error("Upload failed")
        }
      })

      xhr.addEventListener("error", () => {
        entry.error("Upload failed")
      })

      xhr.open("PUT", url, true)
      xhr.send(entry.file)
    })
  }
}

// Audio preview with waveform visualization and playback
const AudioPreview = {
  mounted() {
    this.canvas = this.el.querySelector('canvas')
    this.audio = this.el.querySelector('audio')
    this.playBtn = this.el.querySelector('[data-action="play"]')
    this.stopBtn = this.el.querySelector('[data-action="stop"]')
    this.timeDisplay = this.el.querySelector('[data-time]')
    this.ctx = this.canvas?.getContext('2d')
    this.waveformData = null
    this.isPlaying = false
    this.duration = 0

    // Play/pause button
    this.playBtn?.addEventListener('click', () => this.togglePlay())

    // Stop button
    this.stopBtn?.addEventListener('click', () => this.stop())

    // Click on waveform to seek
    this.canvas?.addEventListener('click', (e) => this.seek(e))

    // Update playhead during playback
    this.audio?.addEventListener('timeupdate', () => this.updatePlayhead())
    this.audio?.addEventListener('ended', () => this.onEnded())

    // Spacebar to toggle play/pause
    this.keyHandler = (e) => {
      if (e.code === 'Space' && e.target.tagName !== 'INPUT' && e.target.tagName !== 'TEXTAREA') {
        e.preventDefault()
        this.togglePlay()
      }
    }
    document.addEventListener('keydown', this.keyHandler)

    // Get the entry ref from element id (audio-preview-{ref})
    const entryRef = this.el.id.replace('audio-preview-', '')
    this.entryRef = entryRef

    // Try to get file from stored uploads (with retry for race condition)
    this.tryLoadFile(entryRef, 0)
  },

  tryLoadFile(entryRef, attempt) {
    const file = window.uploadedFiles[entryRef]
    if (file) {
      this.fileSelected(file)
    } else if (attempt < 10) {
      // Retry a few times with delay
      setTimeout(() => this.tryLoadFile(entryRef, attempt + 1), 100)
    }
  },

  // Called from file input onchange
  fileSelected(file) {
    if (!file) return

    // Create Object URL for playback
    this.objectUrl = URL.createObjectURL(file)

    if (this.audio) {
      this.audio.src = this.objectUrl
    }

    // Decode and draw waveform
    const reader = new FileReader()
    reader.onload = (e) => this.decodeAndDraw(e.target.result)
    reader.readAsArrayBuffer(file)
  },

  decodeAndDraw(arrayBuffer) {
    const audioCtx = new (window.AudioContext || window.webkitAudioContext)()
    audioCtx.decodeAudioData(arrayBuffer, (buffer) => {
      this.duration = buffer.duration
      this.waveformData = this.extractWaveformData(buffer)
      this.drawWaveform()
      this.updateTimeDisplay()
      audioCtx.close()
    })
  },

  extractWaveformData(buffer) {
    const data = buffer.getChannelData(0)
    const width = this.canvas.width
    const step = Math.ceil(data.length / width)
    const result = []

    for (let i = 0; i < width; i++) {
      let min = 1.0, max = -1.0
      for (let j = 0; j < step; j++) {
        const idx = i * step + j
        if (idx < data.length) {
          const sample = data[idx]
          if (sample < min) min = sample
          if (sample > max) max = sample
        }
      }
      result.push({ min, max })
    }
    return result
  },

  drawWaveform(progressX = 0) {
    if (!this.canvas || !this.ctx || !this.waveformData) return

    const width = this.canvas.width
    const height = this.canvas.height
    const amp = height / 2

    // Clear canvas
    this.ctx.fillStyle = '#0f0a19'
    this.ctx.fillRect(0, 0, width, height)

    // Draw waveform bars
    for (let i = 0; i < this.waveformData.length; i++) {
      const { min, max } = this.waveformData[i]
      // Played portion in bright purple, rest in muted purple
      this.ctx.strokeStyle = i < progressX ? '#a855f7' : '#4c1d95'
      this.ctx.beginPath()
      this.ctx.moveTo(i, amp + min * amp)
      this.ctx.lineTo(i, amp + max * amp)
      this.ctx.stroke()
    }

    // Draw playhead line
    if (progressX > 0 && progressX < width) {
      this.ctx.strokeStyle = '#ffffff'
      this.ctx.lineWidth = 2
      this.ctx.beginPath()
      this.ctx.moveTo(progressX, 0)
      this.ctx.lineTo(progressX, height)
      this.ctx.stroke()
      this.ctx.lineWidth = 1
    }
  },

  togglePlay() {
    if (!this.audio || !this.audio.src) return
    if (this.isPlaying) {
      this.audio.pause()
      this.isPlaying = false
      if (this.playBtn) this.playBtn.innerHTML = '<svg class="w-4 h-4" fill="currentColor" viewBox="0 0 24 24"><path d="M8 5v14l11-7z" /></svg>'
    } else {
      this.audio.play()
      this.isPlaying = true
      if (this.playBtn) this.playBtn.innerHTML = '<svg class="w-4 h-4" fill="currentColor" viewBox="0 0 24 24"><rect x="6" y="5" width="4" height="14" rx="1" /><rect x="14" y="5" width="4" height="14" rx="1" /></svg>'
    }
  },

  stop() {
    if (!this.audio) return
    this.audio.pause()
    this.audio.currentTime = 0
    this.isPlaying = false
    if (this.playBtn) this.playBtn.innerHTML = '<svg class="w-4 h-4" fill="currentColor" viewBox="0 0 24 24"><path d="M8 5v14l11-7z" /></svg>'
    this.updatePlayhead()
  },

  seek(e) {
    if (!this.audio || !this.duration || !this.audio.src) return
    const rect = this.canvas.getBoundingClientRect()
    const x = e.clientX - rect.left
    const ratio = x / rect.width
    this.audio.currentTime = ratio * this.duration
    this.updatePlayhead()
  },

  updatePlayhead() {
    if (!this.audio || !this.duration) return
    const ratio = this.audio.currentTime / this.duration
    const progressX = Math.floor(ratio * this.canvas.width)
    this.drawWaveform(progressX)
    this.updateTimeDisplay()
  },

  updateTimeDisplay() {
    if (this.timeDisplay && this.duration) {
      const current = this.formatTime(this.audio?.currentTime || 0)
      const total = this.formatTime(this.duration)
      this.timeDisplay.textContent = `${current} / ${total}`
    }
  },

  onEnded() {
    this.isPlaying = false
    if (this.playBtn) this.playBtn.innerHTML = '<svg class="w-4 h-4" fill="currentColor" viewBox="0 0 24 24"><path d="M8 5v14l11-7z" /></svg>'
  },

  formatTime(seconds) {
    const m = Math.floor(seconds / 60)
    const s = Math.floor(seconds % 60)
    return `${m}:${s.toString().padStart(2, '0')}`
  },

  destroyed() {
    if (this.objectUrl) {
      URL.revokeObjectURL(this.objectUrl)
    }
    // Clean up stored file
    if (this.entryRef && window.uploadedFiles[this.entryRef]) {
      delete window.uploadedFiles[this.entryRef]
    }
    // Remove keyboard listener
    if (this.keyHandler) {
      document.removeEventListener('keydown', this.keyHandler)
    }
  }
}

// Custom hooks
const Hooks = {
  AudioPreview,
  ...colocatedHooks
}

const liveSocket = new LiveSocket("/live", Socket, {
  longPollFallbackMs: 2500,
  params: {_csrf_token: csrfToken},
  hooks: Hooks,
  uploaders: Uploaders,
})

// Show progress bar on live navigation and form submits
topbar.config({barColors: {0: "#29d"}, shadowColor: "rgba(0, 0, 0, .3)"})
window.addEventListener("phx:page-loading-start", _info => topbar.show(300))
window.addEventListener("phx:page-loading-stop", _info => topbar.hide())

// connect if there are any LiveViews on the page
liveSocket.connect()

// Handle file downloads from LiveView
window.addEventListener("phx:download", (event) => {
  const { data, filename, content_type } = event.detail;
  const byteCharacters = atob(data);
  const byteNumbers = new Array(byteCharacters.length);
  for (let i = 0; i < byteCharacters.length; i++) {
    byteNumbers[i] = byteCharacters.charCodeAt(i);
  }
  const byteArray = new Uint8Array(byteNumbers);
  const blob = new Blob([byteArray], { type: content_type });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
})

// expose liveSocket on window for web console debug logs and latency simulation:
// >> liveSocket.enableDebug()
// >> liveSocket.enableLatencySim(1000)  // enabled for duration of browser session
// >> liveSocket.disableLatencySim()
window.liveSocket = liveSocket

// The lines below enable quality of life phoenix_live_reload
// development features:
//
//     1. stream server logs to the browser console
//     2. click on elements to jump to their definitions in your code editor
//
if (process.env.NODE_ENV === "development") {
  window.addEventListener("phx:live_reload:attached", ({detail: reloader}) => {
    // Enable server log streaming to client.
    // Disable with reloader.disableServerLogs()
    reloader.enableServerLogs()

    // Open configured PLUG_EDITOR at file:line of the clicked element's HEEx component
    //
    //   * click with "c" key pressed to open at caller location
    //   * click with "d" key pressed to open at function component definition location
    let keyDown
    window.addEventListener("keydown", e => keyDown = e.key)
    window.addEventListener("keyup", _e => keyDown = null)
    window.addEventListener("click", e => {
      if(keyDown === "c"){
        e.preventDefault()
        e.stopImmediatePropagation()
        reloader.openEditorAtCaller(e.target)
      } else if(keyDown === "d"){
        e.preventDefault()
        e.stopImmediatePropagation()
        reloader.openEditorAtDef(e.target)
      }
    }, true)

    window.liveReloader = reloader
  })
}

