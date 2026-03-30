%{
  title: "Come registrare audio pulito in Audacity",
  description: "Consigli pratici su scelta del microfono, regolazione del gain, trattamento acustico e impostazioni di Audacity per ottenere registrazioni pulite fin dall'inizio.",
  date: ~D[2026-03-05]
}
---

La regola d'oro dell'audio è semplice: se registri male, nessun plugin ti salva. La buona notizia? Bastano 15 minuti di preparazione per risparmiare ore di editing e ottenere risultati nettamente migliori.

Ecco tutto quello che serve per registrare audio pulito in Audacity.

## Scegli il microfono giusto

Se registri in un ufficio o in una stanza di casa, **usa un microfono dinamico**. Modelli come il Rode PodMic, lo Shure SM58 o il Samson Q2U costano poco e rifiutano naturalmente il rumore di fondo. Sono meno sensibili dei microfoni a condensatore — e in uno spazio non trattato è esattamente quello che vuoi.

I microfoni a condensatore catturano più dettagli, ma raccolgono anche ogni clic della tastiera, ronzio di ventilatore e auto di passaggio. Riservali a stanze trattate acusticamente.

Qualunque microfono tu scelga, assicurati che abbia un **pattern polare cardioide**. I cardioidi rifiutano il suono dai lati e da dietro, tenendo il rumore della stanza fuori dalla registrazione.

## Avvicinati al microfono

È il consiglio che fa più differenza in assoluto. Posizionati a **15-30 cm** dal microfono. Un riferimento rapido: circa quattro dita dalla capsula.

Con un dinamico puoi avvicinarti ancora di più (5-8 cm) per un suono più caldo e intimo. Ma non premere le labbra contro il microfono — causa un accumulo di bassi per l'effetto di prossimità.

**Orienta il microfono a circa 30 gradi** rispetto alla bocca invece di parlarci direttamente davanti. Riduce le plosive (quei colpi secchi su P, B e T) mantenendo un suono pieno e naturale. Un filtro antipop aiuta molto — prendine uno se non ce l'hai già.

## Configura Audacity

Prima di premere registra, controlla queste impostazioni:

**Frequenza di campionamento:** 44.100 Hz è lo standard per la voce. Usa 48.000 Hz se l'audio accompagna un video. Andare oltre non serve a nulla per la voce.

**Profondità di bit:** Tieni il valore predefinito di Audacity a 32-bit float per registrazione e montaggio. Ti dà un margine enorme e un fruscio bassissimo. Esporta a 16-bit per il file finale.

**Canali:** Registra in **1 (Mono)**. La voce è una sorgente mono — lo stereo raddoppia il peso del file senza alcun vantaggio. Un'ora in mono pesa circa 310 MB, contro 620 MB in stereo.

## Regola bene il gain

Qui la maggior parte dei principianti sbaglia. Regola i livelli sull'**hardware** — la manopola del gain sulla tua interfaccia audio o sul microfono USB — non nel software.

Ecco come fare:

1. In Audacity, clicca sull'icona del microfono nella barra dei meter e seleziona **Start Monitoring**
2. Parla al tuo volume normale
3. Regola il gain di ingresso finché i picchi stanno tra **-12 dBFS e -6 dBFS**
4. Lascia margine per quando ridi o ti entusiasmi — i picchi non devono mai toccare 0 dBFS

![Allarga il meter di ingresso di Audacity trascinando il bordo](/images/blog/resize-meter.png)

**Consiglio:** Trascina il bordo del meter di ingresso per allargarlo — di default è minuscolo e non si legge niente.

Il clipping digitale è una distorsione aggressiva e irrecuperabile. Meglio registrare un po' troppo piano che un po' troppo forte.

**Errore classico:** registrare troppo piano e poi alzare tutto con Amplify o Normalize in fase di montaggio. Così alzi anche il fruscio. Meglio catturare un buon livello fin dall'inizio.

## Tratta la stanza (senza spendere una fortuna)

Non serve uno studio professionale. Pochi accorgimenti fanno una differenza enorme:

**Gratis:**
- Registra in una **stanza piccola e arredata** — tappeto, tende, divano. Un armadio pieno di vestiti è una delle migliori cabine vocali improvvisate che puoi avere
- Appendi **coperte spesse o piumoni** sulle pareti dietro e ai lati del microfono
- Metti un **tappeto sui pavimenti duri** per ridurre le riflessioni
- Sistema **scaffali pieni di libri** di fronte al microfono — diffondono il suono in modo naturale

**Economico (25-30 € a pannello):**
- Costruisci pannelli acustici fai-da-te con telai in legno, lana di roccia e tessuto
- Mettili nei punti di prima riflessione: dietro il microfono, dietro di te e sulle pareti laterali

**Ordine di priorità:** prima dietro il microfono, poi dietro di te, poi i lati, infine il soffitto.

## Elimina il rumore prima di registrare

La riduzione del rumore in post-produzione è sempre un compromesso. Molto meglio eliminarlo alla fonte:

- **Spegni** condizionatore, ventilatori, riscaldamento e tutto ciò che fa rumore nella stanza
- **Chiudi finestre e porte** — anche una finestra socchiusa lascia entrare traffico e vento
- **Spegni** i dispositivi elettronici non necessari — monitor, hard disk esterni e computer fissi sono i colpevoli più comuni
- Se devi registrare vicino al computer, punta il **retro del microfono** (il punto di massima attenuazione) verso il computer

**Per ronzii e disturbi elettrici:**
- Collega tutta l'attrezzatura alla **stessa ciabatta** per evitare loop di massa
- Usa **cavi XLR bilanciati** invece del jack da 3,5 mm — rifiutano le interferenze elettromagnetiche
- Con un microfono USB, collegalo direttamente al computer — evita gli hub

Segui queste basi e le tue registrazioni saranno già più pulite della maggior parte dei podcast — prima ancora di aprire un plugin.

Hai già registrato qualcosa che non è venuto bene? Leggi [Come salvare una registrazione podcast venuta male](/blog/fix-bad-podcast-recording) prima di riregistrare.

E quando vorrai rifinirle, [prova Munchy Cow](/). Carica la registrazione e pensiamo noi al resto — rumore di fondo, riverbero, livelli irregolari — senza farti sembrare un robot.
