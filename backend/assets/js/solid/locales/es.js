// Spanish translations for Solid app
export default {
  // Upload zone
  feedTheCow: "Alimenta a la vaca!",
  veryHungry: "Tiene MUCHA hambre de tu audio",
  nomNomNom: "nom nom nom - WAV, MP3, FLAC",
  readyToMunch: "Lista para masticar!",
  estimatedMinutes: "~{minutes} min",
  finalizing: "Finalizando...",
  removeFile: "Quitar archivo",
  uploadAudioFile: "Subir archivo de audio. Haz clic o arrastra un archivo aqui.",

  // Validation errors
  selectAudioFile: "Por favor selecciona un archivo de audio (WAV, MP3, FLAC, etc.)",
  fileTooLarge: "Archivo demasiado grande. Tamano maximo: 500MB.",
  uploadFirst: "Por favor sube un archivo primero",

  // Category toggle
  whatProcessing: "Que estas procesando?",
  voice: "Voz",
  mixedAudio: "Audio Mixto",

  // Mode selector
  processingMode: "Modo de procesamiento",
  natural: "Natural",
  studio: "Estudio",
  aiClean: "Limpieza IA",
  takesLonger: "Tarda mas en procesar",

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
  munchIt: "MASTICALO!",

  // Job progress
  munchMunchMunch: "*munch munch munch*",
  munching: "Masticando...",
  cancel: "Cancelar",

  // Job complete
  mooo: "MUUU!",
  audioReady: "Tu audio esta listo!",
  original: "Original",
  processed: "Procesado",
  download: "Descargar",
  downloadProcessedAudio: "Descargar audio procesado",
  feedMeMore: "Dame mas!",
  uploadAnotherFile: "Subir otro archivo",
  audioComparison: "Comparacion de audio",

  // Job failed
  cowChoked: "La vaca se atraganto!",
  feedHerAgain: "Alimentala de nuevo",

  // Past munchings
  pastMunchings: "Procesamientos Anteriores",
  pastDescription: "Tus archivos procesados de los ultimos 7 dias.",
  noMunchingsYet: "Aun no hay procesamientos. Tus archivos procesados apareceran aqui por 7 dias.",
  today: "Hoy a las {time}",
  yesterday: "Ayer a las {time}",
  daysAgo: "Hace {count} dias a las {time}",

  // Connection status
  connectionRestored: "Conexion restaurada",
  connectionLost: "Conexion perdida. Reconectando...",
  unableToConnect: "No se puede conectar. Por favor actualiza la pagina.",

  // Billing
  notEnoughMinutes: "No tienes suficientes minutos. Actualiza a Pro para mas tiempo de procesamiento.",
  needMinutes: " Necesitas {needed} minutos pero solo tienes {available} disponibles.",
  upgrade: "Actualizar",

  // Error messages
  unknownError: "Error desconocido",
  formatNotSupported: "Formato de audio no soportado. Intenta convertir a WAV o MP3.",
  noAudioFound: "No se encontro audio en el archivo.",
  couldNotReadFile: "No se pudo leer el archivo de audio. Puede estar corrupto.",
  processingTooLong: "El procesamiento tardo demasiado. Intenta con un archivo mas corto.",
  couldNotAccessFile: "No se pudo acceder al archivo. Por favor vuelve a subirlo.",
  failedToCreateOutput: "Error al crear el archivo de salida.",

  // Processing stages
  processing: "Procesando...",
  stages: {
    decoding: "Leyendo audio...",
    filters: "Cortando ruido y siseo...",
    input_gain: "Equilibrando niveles...",
    analyzing_reverb: "Detectando sonido de sala...",
    dereverb: "Eliminando eco de sala...",
    analyzing_noise: "Buscando ruido de fondo...",
    denoise: "Limpiando ruido...",
    spectral_gate: "Silenciando partes quietas...",
    analyzing_peaks: "Buscando tonos asperos...",
    peak_attenuation: "Suavizando tonos asperos...",
    expander: "Abriendo dinamicas...",
    compressor: "Nivelando...",
    analyzing_eq: "Revisando el tono...",
    fixeq: "Arreglando zonas turbias...",
    deesser: "Domando las S...",
    saturation: "Agregando calidez...",
    buttercomp: "Unificando...",
    analyzing_enhance: "Optimizando presencia...",
    enhanceeq: "Iluminando...",
    radio: "Pulido de broadcast...",
    tape: "Agregando sensacion analogica...",
    analyzing_levels: "Midiendo volumen...",
    output: "Limitacion final...",
    encoding: "Guardando tu archivo...",
    completed: "Listo!"
  }
};
