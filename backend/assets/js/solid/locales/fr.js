// French translations for Solid app
export default {
  // Upload zone
  feedTheCow: "Nourrissez la vache !",
  veryHungry: "Elle a TRÈS faim de votre audio",
  nomNomNom: "miam miam miam - WAV, MP3, FLAC",
  readyToMunch: "Prête à mâcher !",
  estimatedTime: "~{time}",
  finalizing: "Finalisation...",
  removeFile: "Supprimer le fichier",
  uploadAudioFile: "Téléchargez un fichier audio. Cliquez ou déposez un fichier ici.",

  // Validation errors
  selectAudioFile: "Veuillez sélectionner un fichier audio (WAV, MP3, FLAC, etc.)",
  fileTooLarge: "Fichier trop volumineux. Taille maximale : 500 Mo.",
  uploadFirst: "Veuillez d'abord télécharger un fichier",

  // AI Clean
  aiClean: "Nettoyage IA",
  takesLonger: "Nettoie le bruit avec IA. Prend plus de temps",
  centerAudio: "Centrer Audio",
  centerAudioTooltip: "Mixage de l'audio en mono",

  // Strength knob
  strength: "Intensité",
  subtle: "Léger",
  balanced: "Équilibré",
  intense: "Intense",

  // Steps indicator
  feed: "Nourrir",
  munch: "Mâcher",
  enjoy: "Profiter",

  // Main controls
  munchIt: "MÂCHE-LE !",

  // Job progress
  munchMunchMunch: "*miam miam miam*",
  munching: "En train de mâcher...",
  cancel: "Annuler",

  // Job complete
  mooo: "MEUH !",
  audioReady: "Votre audio est prêt !",
  original: "Original",
  processed: "Traité",
  download: "Télécharger",
  downloadProcessedAudio: "Télécharger l'audio traité",
  feedMeMore: "Donnez-m'en plus !",
  uploadAnotherFile: "Télécharger un autre fichier",
  audioComparison: "Comparaison audio",

  // Job failed
  cowChoked: "La vache s'est étouffée !",
  feedHerAgain: "Nourrissez-la à nouveau",

  // Past munchings
  pastMunchings: "Mâchages Précédents",
  pastDescription: "Vos fichiers mâchés des 7 derniers jours.",
  noMunchingsYet: "Pas encore de mâchages. Vos fichiers mâchés apparaîtront ici pendant 7 jours.",
  today: "Aujourd'hui à {time}",
  yesterday: "Hier à {time}",
  daysAgo: "Il y a {count} jours à {time}",

  // Connection status
  connectionRestored: "Connexion rétablie",
  connectionLost: "Connexion perdue. Reconnexion...",
  unableToConnect: "Impossible de se connecter. Veuillez rafraîchir la page.",

  // Billing
  notEnoughTime: "Temps insuffisant. Passez au Munch Plan ou achetez un Snack pour plus de temps.",
  needTime: " Vous avez besoin de {needed} mais n'en avez que {available} disponibles.",
  upgrade: "Voir les options",

  // Error messages
  unknownError: "Erreur inconnue",
  formatNotSupported: "Format audio non supporté. Essayez de convertir en WAV ou MP3.",
  noAudioFound: "Aucun audio trouvé dans le fichier.",
  couldNotReadFile: "Impossible de lire le fichier audio. Il est peut-être corrompu.",
  processingTooLong: "Le traitement a pris trop de temps. Essayez un fichier plus court.",
  couldNotAccessFile: "Impossible d'accéder au fichier. Veuillez le télécharger à nouveau.",
  failedToCreateOutput: "Échec de la création du fichier de sortie.",

  // Processing stages
  processing: "Traitement...",
  stages: {
    waiting: "En attente d'un créneau...",
    decoding: "Lecture audio...",
    filters: "Coupe des grondements et sifflements...",
    input_gain: "Équilibrage des niveaux...",
    analyzing_reverb: "Détection du son de la pièce...",
    dereverb: "Suppression de l'écho de la pièce...",
    analyzing_noise: "Recherche du bruit de fond...",
    denoise: "Nettoyage du bruit...",
    ai_denoise: "Nettoyage IA de la voix...",
    spectral_gate: "Mise en sourdine des parties calmes...",
    peakcomp: "Nivellement...",
    analyzing_eq: "Vérification du ton...",
    fixeq: "Correction des zones boueuses...",
    deesser: "Domptage des S...",
    saturation: "Ajout de chaleur...",
    buttercomp: "Assemblage...",
    analyzing_enhance: "Optimisation de la présence...",
    enhanceeq: "Éclaircissement...",
    radio: "Polissage broadcast...",
    fetcomp: "Compression finale...",
    tape: "Ajout de sensation analogique...",
    analyzing_levels: "Mesure du volume...",
    output: "Limitation finale...",
    completed: "Terminé !"
  }
};
