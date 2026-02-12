// Spanish translations for Solid app
export default {
  // Upload zone
  feedTheCow: "¡Alimenta a la vaca!",
  veryHungry: "Tiene MUCHA hambre de tu audio",
  nomNomNom: "nom nom nom - WAV, MP3, FLAC",
  readyToMunch: "¡Lista para masticar!",
  estimatedTime: "~{time}",
  finalizing: "Finalizando...",
  removeFile: "Quitar archivo",
  uploadAudioFile: "Subir archivo de audio. Haz clic o arrastra un archivo aquí.",

  // Validation errors
  selectAudioFile: "Por favor selecciona un archivo de audio (WAV, MP3, FLAC, etc.)",
  fileTooLarge: "Archivo demasiado grande. Tamaño máximo: 500MB.",
  uploadFirst: "Por favor sube un archivo primero",

  // AI Clean
  aiClean: "Limpieza IA",
  takesLonger: "Limpia el ruido con IA. Tarda más en procesar",
  centerAudio: "Centrar Audio",
  centerAudioTooltip: "Mezcla el audio a mono",

  // Strength knob
  strength: "Intensidad",
  subtle: "Sutil",
  balanced: "Equilibrado",
  intense: "Intenso",

  // Steps indicator
  feed: "Alimentar",
  munch: "Masticar",
  enjoy: "Disfrutar",

  // Main controls
  munchIt: "¡MASTÍCALO!",

  // Job progress
  munchMunchMunch: "*ñam ñam ñam*",
  munching: "Masticando...",
  cancel: "Cancelar",

  // Job complete
  mooo: "¡MUUU!",
  audioReady: "¡Tu audio está listo!",
  original: "Original",
  processed: "Procesado",
  download: "Descargar",
  downloadProcessedAudio: "Descargar audio procesado",
  feedMeMore: "¡Dame más!",
  uploadAnotherFile: "Subir otro archivo",
  audioComparison: "Comparación de audio",

  // Job failed
  cowChoked: "¡La vaca se atragantó!",
  feedHerAgain: "Aliméntala de nuevo",

  // Past munchings
  pastMunchings: "Masticaciones Anteriores",
  pastDescription: "Tus archivos masticados de los últimos 7 días.",
  noMunchingsYet: "Aún no hay masticaciones. Tus archivos masticados aparecerán aquí por 7 días.",
  today: "Hoy a las {time}",
  yesterday: "Ayer a las {time}",
  daysAgo: "Hace {count} días a las {time}",

  // Connection status
  connectionRestored: "Conexión restaurada",
  connectionLost: "Conexión perdida. Reconectando...",
  unableToConnect: "No se puede conectar. Por favor actualiza la página.",

  // Billing
  notEnoughTime: "No tienes suficiente tiempo. Pasa al Munch Plan o compra un Snack para más tiempo.",
  needTime: " Necesitas {needed} pero solo tienes {available} disponibles.",
  upgrade: "Ver opciones",

  // Error messages
  unknownError: "Error desconocido",
  formatNotSupported: "Formato de audio no soportado. Intenta convertir a WAV o MP3.",
  noAudioFound: "No se encontró audio en el archivo.",
  couldNotReadFile: "No se pudo leer el archivo de audio. Puede estar corrupto.",
  processingTooLong: "El procesamiento tardó demasiado. Intenta con un archivo más corto.",
  couldNotAccessFile: "No se pudo acceder al archivo. Por favor vuelve a subirlo.",
  failedToCreateOutput: "Error al crear el archivo de salida.",

  // Processing stages
  processing: "Procesando...",
  stages: {
    waiting: "Esperando turno...",
    decoding: "Leyendo audio...",
    filters: "Cortando ruido y siseo...",
    input_gain: "Equilibrando niveles...",
    analyzing_reverb: "Detectando sonido de sala...",
    dereverb: "Eliminando eco de sala...",
    analyzing_noise: "Buscando ruido de fondo...",
    denoise: "Limpiando ruido...",
    ai_denoise: "Limpieza IA de voz...",
    spectral_gate: "Silenciando partes quietas...",
    peakcomp: "Nivelando...",
    analyzing_eq: "Revisando el tono...",
    fixeq: "Arreglando zonas turbias...",
    deesser: "Domando las S...",
    saturation: "Agregando calidez...",
    buttercomp: "Unificando...",
    analyzing_enhance: "Optimizando presencia...",
    enhanceeq: "Iluminando...",
    radio: "Pulido de broadcast...",
    fetcomp: "Compresión final...",
    tape: "Agregando sensación analógica...",
    analyzing_levels: "Midiendo volumen...",
    output: "Limitación final...",
    completed: "¡Listo!"
  }
};
