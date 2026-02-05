// French translations for Solid app
export default {
  // Upload zone
  feedTheCow: "Nourrissez la vache !",
  veryHungry: "Elle a TRES faim de votre audio",
  nomNomNom: "miam miam miam - WAV, MP3, FLAC",
  readyToMunch: "Prete a macher !",
  estimatedTime: "~{time}",
  finalizing: "Finalisation...",
  removeFile: "Supprimer le fichier",
  uploadAudioFile: "Telechargez un fichier audio. Cliquez ou deposez un fichier ici.",

  // Validation errors
  selectAudioFile: "Veuillez selectionner un fichier audio (WAV, MP3, FLAC, etc.)",
  fileTooLarge: "Fichier trop volumineux. Taille maximale : 500 Mo.",
  uploadFirst: "Veuillez d'abord telecharger un fichier",

  // Category toggle
  whatProcessing: "Que traitez-vous ?",
  voice: "Voix",
  mixedAudio: "Audio Mixte",

  // Mode selector
  processingMode: "Mode de traitement",
  natural: "Naturel",
  studio: "Studio",
  aiClean: "Nettoyage IA",
  takesLonger: "Prend plus de temps",

  // Strength knob
  strength: "Intensite",
  subtle: "Leger",
  balanced: "Equilibre",
  intense: "Intense",

  // Steps indicator
  feed: "Nourrir",
  munch: "Macher",
  enjoy: "Profiter",

  // Main controls
  munchIt: "MACHE-LE !",

  // Job progress
  munchMunchMunch: "*miam miam miam*",
  munching: "En train de macher...",
  cancel: "Annuler",

  // Job complete
  mooo: "MEUH !",
  audioReady: "Votre audio est pret !",
  original: "Original",
  processed: "Traite",
  download: "Telecharger",
  downloadProcessedAudio: "Telecharger l'audio traite",
  feedMeMore: "Donnez-m'en plus !",
  uploadAnotherFile: "Telecharger un autre fichier",
  audioComparison: "Comparaison audio",

  // Job failed
  cowChoked: "La vache s'est etouffee !",
  feedHerAgain: "Nourrissez-la a nouveau",

  // Past munchings
  pastMunchings: "Mâchages Précédents",
  pastDescription: "Vos fichiers mâchés des 7 derniers jours.",
  noMunchingsYet: "Pas encore de mâchages. Vos fichiers mâchés apparaîtront ici pendant 7 jours.",
  today: "Aujourd'hui a {time}",
  yesterday: "Hier a {time}",
  daysAgo: "Il y a {count} jours a {time}",

  // Connection status
  connectionRestored: "Connexion retablie",
  connectionLost: "Connexion perdue. Reconnexion...",
  unableToConnect: "Impossible de se connecter. Veuillez rafraichir la page.",

  // Billing
  notEnoughTime: "Temps insuffisant. Passez au Munch Plan ou achetez un Snack pour plus de temps.",
  needTime: " Vous avez besoin de {needed} mais n'en avez que {available} disponibles.",
  upgrade: "Voir les options",

  // Error messages
  unknownError: "Erreur inconnue",
  formatNotSupported: "Format audio non supporte. Essayez de convertir en WAV ou MP3.",
  noAudioFound: "Aucun audio trouve dans le fichier.",
  couldNotReadFile: "Impossible de lire le fichier audio. Il est peut-etre corrompu.",
  processingTooLong: "Le traitement a pris trop de temps. Essayez un fichier plus court.",
  couldNotAccessFile: "Impossible d'acceder au fichier. Veuillez le telecharger a nouveau.",
  failedToCreateOutput: "Echec de la creation du fichier de sortie.",

  // Processing stages
  processing: "Traitement...",
  stages: {
    waiting: "En attente d'un creneau...",
    decoding: "Lecture audio...",
    filters: "Coupe des grondements et sifflements...",
    input_gain: "Equilibrage des niveaux...",
    analyzing_reverb: "Detection du son de la piece...",
    dereverb: "Suppression de l'echo de la piece...",
    analyzing_noise: "Recherche du bruit de fond...",
    denoise: "Nettoyage du bruit...",
    ai_denoise: "Nettoyage IA de la voix...",
    spectral_gate: "Mise en sourdine des parties calmes...",
    analyzing_peaks: "Recherche des tons agressifs...",
    peak_attenuation: "Adoucissement des tons agressifs...",
    expander: "Ouverture des dynamiques...",
    compressor: "Nivellement...",
    analyzing_eq: "Verification du ton...",
    fixeq: "Correction des zones boueuses...",
    deesser: "Domptage des S...",
    saturation: "Ajout de chaleur...",
    buttercomp: "Assemblage...",
    analyzing_enhance: "Optimisation de la presence...",
    enhanceeq: "Eclaircissement...",
    radio: "Polissage broadcast...",
    tape: "Ajout de sensation analogique...",
    analyzing_levels: "Mesure du volume...",
    output: "Limitation finale...",
    encoding: "Sauvegarde de votre fichier...",
    completed: "Termine !"
  }
};
