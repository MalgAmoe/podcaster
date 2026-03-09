%{
  title: "LUFS per podcaster: perché i tuoi episodi suonano diversi su ogni piattaforma",
  description: "Il tuo podcast suona bene sul computer ma basso su Spotify e forte su Apple Podcasts. Ecco il numero che spiega tutto — e come correggerlo.",
  date: ~D[2026-03-09]
}
---

Pubblichi un episodio. Sul computer suona bene. Poi un ascoltatore ti scrive: "Hey, il tuo show è davvero basso — devo alzare il volume al massimo." Un altro ascoltatore su un'altra app dice che suona bene. Cosa succede?

La risposta è un'unità di misura chiamata LUFS, e una volta che la capisci, il tuo podcast suonerà coerente ovunque.

## Cos'è il LUFS?

LUFS sta per Loudness Units relative to Full Scale. Misura quanto l'audio *suona* forte all'orecchio umano — non solo quanto sono alti i picchi della forma d'onda.

Questa distinzione conta. Un sussurro e un grido possono avere lo stesso livello di picco se il grido è breve e il sussurro è sostenuto. Ma non suonano ugualmente forti. Il LUFS tiene conto di questo misurando il loudness nel tempo, ponderato per corrispondere all'udito umano.

Il livello di picco ti dice l'onda più alta dell'oceano. Il LUFS ti dice quanto è agitato il mare.

## Perché i podcaster devono preoccuparsene

Ogni grande piattaforma podcast ha un obiettivo di loudness. Se il tuo episodio non corrisponde, la piattaforma lo aggiusta — a volte bene, a volte male.

| Piattaforma | Obiettivo | Cosa succede se non ci sei |
|----------|--------|-----------------------------|
| **Spotify** | -14 LUFS | Normalizza a -14. Troppo basso = alzato (col rumore). Troppo forte = abbassato. |
| **Apple Podcasts** | -16 LUFS | Sound Check normalizza se attivato dall'ascoltatore. Inconsistente. |
| **YouTube** | -14 LUFS | Normalizza verso il basso ma non verso l'alto. Ciò che è basso resta basso. |
| **Overcast** | Voice Boost | Ha il suo processing. Applica compressione e boost di volume in ogni caso. |

Il problema: se masterizzi a -20 LUFS (comune per registrazioni non trattate) e un ascoltatore è su Spotify, il tuo episodio viene alzato — e tutto il rumore di fondo sale con esso.

## L'obiettivo di -16 LUFS

La maggior parte dei professionisti del podcast raccomanda **-16 LUFS**. È diventato lo standard largamente adottato per il parlato:

- Su Spotify (obiettivo -14), il tuo episodio viene abbassato di 2 dB — appena percettibile.
- Su Apple Podcasts (obiettivo -16), corrisponde perfettamente.
- Su YouTube (obiettivo -14), viene abbassato leggermente — nessun problema.
- Lascia abbastanza margine perché il tuo audio non suoni schiacciato o sovra-compresso.

Questo conta di più per la voce che per la musica. La musica è già pesantemente compressa e masterizzata, quindi spingerla a -14 LUFS va bene. La voce è più dinamica — ha momenti naturalmente forti e deboli che vuoi preservare. Spingere la voce a -14 LUFS richiede più compressione, che può farla suonare piatta e innaturale. -16 dà alla voce spazio per respirare pur restando presente e professionale.

## True peak: l'altro numero che conta

Il LUFS misura il loudness medio. Il true peak (dBTP) misura il livello massimo assoluto, inclusi i picchi che si verificano tra i campioni.

Se il tuo true peak è a 0 dBTP (massimo), quando la piattaforma ri-codifica il tuo file o il dispositivo dell'ascoltatore ricostruisce il segnale, quei picchi possono superare 0 dB e distorcere. Lo senti come un breve crepitio sulle consonanti forti.

**Il limite sicuro: -1 dBTP.** Imposta il tetto del tuo limiter a -1.0 dBTP. Questo dà abbastanza margine per la conversione di codec e la riproduzione su qualsiasi dispositivo.

## Come misurare il LUFS

Ti serve un loudness meter:

- **Youlean Loudness Meter** (plugin gratuito) — Mostra LUFS integrato, a breve termine, momentaneo, true peak e loudness range. La migliore opzione gratuita.
- **ffmpeg** (gratuito, riga di comando) — `ffmpeg -i tuo_file.mp3 -af loudnorm=print_format=json -f null -` ti dà il LUFS integrato e il true peak.
- **La maggior parte dei DAW** ha una misurazione del loudness integrata o supporta plugin di misurazione.

## Come raggiungere il tuo obiettivo

### Approccio manuale

1. **Processa il tuo audio prima** — riduzione del rumore, compressione, EQ, tutto il resto prima del loudness.
2. **Aggiungi un limiter** in uscita. Imposta il tetto a -1.0 dBTP.
3. **Normalizza a -16 LUFS.** La maggior parte dei DAW ha una funzione di normalizzazione del loudness.
4. **Verifica con un meter.** Riproduci l'intero episodio e leggi il valore di LUFS integrato. Dovrebbe essere entro 1 dB dal tuo obiettivo.

### Automatico

Strumenti come Auphonic, Munchy Cow e altri possono gestire l'obiettivo di loudness automaticamente. Il vantaggio di farlo come parte di una catena di processing completa (rimozione del rumore, compressione, EQ) piuttosto che normalizzare semplicemente l'audio grezzo è che non stai alzando le cose brutte insieme a quelle buone.

## Gli errori più comuni

### Normalizzare senza processare prima

Se il tuo audio grezzo è a -24 LUFS e lo normalizzi semplicemente a -16, stai alzando tutto di 8 dB — inclusi il rumore, il riverbero e i click della bocca. Processa prima, normalizza per ultimo.

### Sovra-comprimere per raggiungere il numero

La compressione pesante può portarti a -16 LUFS, ma suona piatto e affaticante. Usa una compressione leggera (ratio da 2:1 a 4:1) e lascia che il normalizzatore alzi il livello complessivo naturalmente.

### Ignorare il true peak

Hai raggiunto -16 LUFS ma il tuo true peak è a 0 dBTP. Gli ascoltatori sentono distorsione su certi dispositivi. Usa sempre un limiter di picco reale impostato a -1.0 dBTP come ultima cosa nella tua catena.

### Loudness inconsistente da episodio a episodio

L'episodio 1 è a -14 LUFS. L'episodio 2 è a -18 LUFS. L'episodio 3 è a -15 LUFS. Gli ascoltatori devono regolare il volume a ogni episodio. Scegli un obiettivo e mantienilo.

## Riferimento rapido

- **Obiettivo: -16 LUFS** loudness integrato
- **Tetto di true peak: -1.0 dBTP**
- **Processa prima di normalizzare** — non alzare mai semplicemente l'audio grezzo
- **Sii coerente** su ogni episodio

---

Non vuoi impazzire con meter e limiter? [Carica su Munchy Cow](/users/log-in) e il loudness viene gestito automaticamente — insieme alla catena di processing completa che fa sì che suoni davvero bene a quel livello. 3 ore gratis, senza carta di credito.
