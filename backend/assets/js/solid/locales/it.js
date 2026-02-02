// Italian translations for Solid app
export default {
  // Upload zone
  feedTheCow: "Dai da mangiare alla mucca!",
  veryHungry: "Ha MOLTA fame del tuo audio",
  nomNomNom: "gnam gnam gnam - WAV, MP3, FLAC",
  readyToMunch: "Pronta a masticare!",
  estimatedTime: "~{time}",
  finalizing: "Finalizzazione...",
  removeFile: "Rimuovi file",
  uploadAudioFile: "Carica file audio. Clicca o trascina un file qui.",

  // Validation errors
  selectAudioFile: "Seleziona un file audio (WAV, MP3, FLAC, ecc.)",
  fileTooLarge: "File troppo grande. Dimensione massima: 500MB.",
  uploadFirst: "Prima carica un file",

  // Category toggle
  whatProcessing: "Cosa stai elaborando?",
  voice: "Voce",
  mixedAudio: "Audio Misto",

  // Mode selector
  processingMode: "Modalita di elaborazione",
  natural: "Naturale",
  studio: "Studio",
  aiClean: "Pulizia IA",
  takesLonger: "Richiede piu tempo",

  // Strength knob
  strength: "Intensita",
  subtle: "Leggera",
  balanced: "Bilanciata",
  intense: "Intensa",

  // Steps indicator
  feed: "Alimenta",
  munch: "Mastica",
  enjoy: "Goditi",

  // Main controls
  munchIt: "MASTICALO!",

  // Job progress
  munchMunchMunch: "*gnam gnam gnam*",
  munching: "Masticando...",
  cancel: "Annulla",

  // Job complete
  mooo: "MUUU!",
  audioReady: "Il tuo audio e pronto!",
  original: "Originale",
  processed: "Elaborato",
  download: "Scarica",
  downloadProcessedAudio: "Scarica audio elaborato",
  feedMeMore: "Dammi di piu!",
  uploadAnotherFile: "Carica un altro file",
  audioComparison: "Confronto audio",

  // Job failed
  cowChoked: "La mucca si e strozzata!",
  feedHerAgain: "Dalla di nuovo",

  // Past munchings
  pastMunchings: "Elaborazioni Precedenti",
  pastDescription: "I tuoi file elaborati degli ultimi 7 giorni.",
  noMunchingsYet: "Ancora nessuna elaborazione. I tuoi file elaborati appariranno qui per 7 giorni.",
  today: "Oggi alle {time}",
  yesterday: "Ieri alle {time}",
  daysAgo: "{count} giorni fa alle {time}",

  // Connection status
  connectionRestored: "Connessione ripristinata",
  connectionLost: "Connessione persa. Riconnessione...",
  unableToConnect: "Impossibile connettersi. Aggiorna la pagina.",

  // Billing
  notEnoughTime: "Tempo insufficiente. Passa a Pro per piu tempo di elaborazione.",
  needTime: " Hai bisogno di {needed} ma ne hai solo {available} disponibili.",
  upgrade: "Passa a Pro",

  // Error messages
  unknownError: "Errore sconosciuto",
  formatNotSupported: "Formato audio non supportato. Prova a convertire in WAV o MP3.",
  noAudioFound: "Nessun audio trovato nel file.",
  couldNotReadFile: "Impossibile leggere il file audio. Potrebbe essere corrotto.",
  processingTooLong: "L'elaborazione ha impiegato troppo tempo. Prova con un file piu corto.",
  couldNotAccessFile: "Impossibile accedere al file. Ricaricalo.",
  failedToCreateOutput: "Impossibile creare il file di output.",

  // Processing stages
  processing: "Elaborazione...",
  stages: {
    waiting: "In attesa di uno slot...",
    decoding: "Lettura audio...",
    filters: "Taglio rumori e sibili...",
    input_gain: "Bilanciamento livelli...",
    analyzing_reverb: "Rilevamento suono ambiente...",
    dereverb: "Rimozione eco stanza...",
    analyzing_noise: "Ricerca rumore di fondo...",
    denoise: "Pulizia rumore...",
    ai_denoise: "Pulizia IA della voce...",
    spectral_gate: "Silenziamento parti quiete...",
    analyzing_peaks: "Ricerca toni aspri...",
    peak_attenuation: "Ammorbidimento toni aspri...",
    expander: "Apertura dinamiche...",
    compressor: "Livellamento...",
    analyzing_eq: "Controllo del tono...",
    fixeq: "Correzione zone fangose...",
    deesser: "Domatura delle S...",
    saturation: "Aggiunta calore...",
    buttercomp: "Unificazione...",
    analyzing_enhance: "Ottimizzazione presenza...",
    enhanceeq: "Illuminazione...",
    radio: "Rifinitura broadcast...",
    tape: "Aggiunta sensazione analogica...",
    analyzing_levels: "Misurazione volume...",
    output: "Limitazione finale...",
    encoding: "Salvataggio file...",
    completed: "Fatto!"
  }
};
