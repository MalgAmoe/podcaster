%{
  title: "LUFS pour les podcasters : pourquoi tes épisodes sonnent différemment sur chaque plateforme",
  description: "Ton podcast sonne bien sur ton ordi mais trop bas sur Spotify et trop fort sur Apple Podcasts. Voici le chiffre qui explique tout — et comment le corriger.",
  date: ~D[2026-03-09]
}
---

Tu publies un épisode. Il sonne bien sur ton ordinateur. Puis un auditeur t'écrit : « Hé, ton émission est vraiment basse — je dois monter le volume à fond. » Un autre auditeur sur une autre app dit que ça sonne bien. Que se passe-t-il ?

La réponse est une unité de mesure appelée LUFS, et une fois que tu la comprends, ton podcast sonnera de façon cohérente partout.

## Qu'est-ce que le LUFS ?

LUFS signifie Loudness Units relative to Full Scale. Ça mesure le volume *perçu* par l'oreille humaine — pas juste la hauteur des crêtes de la forme d'onde.

Cette distinction compte. Un chuchotement et un cri peuvent avoir le même niveau de crête si le cri est bref et le chuchotement soutenu. Mais ils ne sonnent pas aussi fort. Le LUFS tient compte de ça en mesurant le loudness dans le temps, pondéré pour correspondre à l'audition humaine.

Le niveau de crête te dit la plus haute vague de l'océan. Le LUFS te dit à quel point la mer est agitée.

## Pourquoi les podcasters doivent s'en soucier

Chaque grande plateforme de podcast a une cible de loudness. Si ton épisode ne correspond pas, la plateforme l'ajuste — parfois bien, parfois mal.

| Plateforme | Cible | Ce qui se passe si tu es à côté |
|----------|--------|--------------------------|
| **Spotify** | -14 LUFS | Normalise à -14. Trop bas = monté (avec le bruit). Trop fort = baissé. |
| **Apple Podcasts** | -16 LUFS | Sound Check normalise si activé par l'auditeur. Inconsistant. |
| **YouTube** | -14 LUFS | Normalise vers le bas mais pas vers le haut. Ce qui est bas reste bas. |
| **Overcast** | Voice Boost | A son propre traitement. Applique compression et boost de volume quoi qu'il arrive. |

Le problème : si tu masterises à -20 LUFS (courant pour les enregistrements non traités) et qu'un auditeur est sur Spotify, ton épisode est monté — et tout le bruit de fond monte avec.

## La cible de -16 LUFS

La plupart des professionnels du podcast recommandent **-16 LUFS**. C'est devenu le standard largement adopté pour la voix parlée :

- Sur Spotify (cible -14), ton épisode est baissé de 2 dB — à peine perceptible.
- Sur Apple Podcasts (cible -16), ça correspond parfaitement.
- Sur YouTube (cible -14), c'est baissé légèrement — pas de souci.
- Ça laisse assez de marge pour que ton audio ne sonne pas écrasé ou sur-comprimé.

C'est plus important pour la voix que pour la musique. La musique est déjà fortement comprimée et masterisée, donc la pousser à -14 LUFS passe bien. La voix est plus dynamique — elle a des moments naturellement forts et doux que tu veux préserver. Pousser la voix à -14 LUFS demande plus de compression, ce qui peut la rendre plate et peu naturelle. -16 donne à la voix de l'espace pour respirer tout en restant présente et professionnelle.

## True peak : l'autre chiffre qui compte

Le LUFS mesure le loudness moyen. Le true peak (dBTP) mesure le niveau maximum absolu, y compris les crêtes qui se produisent entre les échantillons.

Si ton true peak est à 0 dBTP (maximum), quand la plateforme ré-encode ton fichier ou que l'appareil de l'auditeur reconstruit le signal, ces crêtes peuvent dépasser 0 dB et distordre. Tu l'entends comme un bref craquement sur les consonnes fortes.

**La limite sûre : -1 dBTP.** Règle le plafond de ton limiteur à -1.0 dBTP. Ça donne assez de marge pour la conversion de codec et la lecture sur n'importe quel appareil.

## Comment mesurer le LUFS

Tu as besoin d'un loudness meter :

- **Youlean Loudness Meter** (plugin gratuit) — Affiche le LUFS intégré, court terme, momentané, true peak et plage de loudness. La meilleure option gratuite.
- **ffmpeg** (gratuit, ligne de commande) — `ffmpeg -i ton_fichier.mp3 -af loudnorm=print_format=json -f null -` te donne le LUFS intégré et le true peak.
- **La plupart des DAW** ont une mesure du loudness intégrée ou supportent des plugins de mesure.

## Comment atteindre ta cible

### Approche manuelle

1. **Traite ton audio d'abord** — réduction de bruit, compression, EQ, tout le reste avant le loudness.
2. **Ajoute un limiteur** en sortie. Règle le plafond à -1.0 dBTP.
3. **Normalise à -16 LUFS.** La plupart des DAW ont une fonction de normalisation du loudness.
4. **Vérifie avec un meter.** Joue l'épisode entier et lis la valeur de LUFS intégré. Ça devrait être à 1 dB près de ta cible.

### Automatique

Des outils comme Auphonic, Munchy Cow et d'autres peuvent gérer le loudness cible automatiquement. L'avantage de le faire dans le cadre d'une chaîne de traitement complète (suppression du bruit, compression, EQ) plutôt que de juste normaliser l'audio brut, c'est que tu ne montes pas le mauvais en même temps que le bon.

## Les erreurs les plus courantes

### Normaliser sans traiter d'abord

Si ton audio brut est à -24 LUFS et que tu le normalises simplement à -16, tu montes tout de 8 dB — y compris le bruit, la réverb et les clics de bouche. Traite d'abord, normalise en dernier.

### Sur-comprimer pour atteindre le chiffre

La compression lourde peut t'amener à -16 LUFS, mais ça sonne plat et fatigant. Utilise une compression douce (ratio de 2:1 à 4:1) et laisse le normaliseur monter le niveau général naturellement.

### Ignorer le true peak

Tu as atteint -16 LUFS mais ton true peak est à 0 dBTP. Les auditeurs entendent de la distorsion sur certains appareils. Utilise toujours un limiteur de crête réelle réglé à -1.0 dBTP comme dernière étape de ta chaîne.

### Loudness inconsistant d'épisode en épisode

L'épisode 1 est à -14 LUFS. L'épisode 2 est à -18 LUFS. L'épisode 3 est à -15 LUFS. Les auditeurs doivent ajuster le volume à chaque épisode. Choisis une cible et tiens-t'y.

## Référence rapide

- **Cible : -16 LUFS** loudness intégré
- **Plafond de true peak : -1.0 dBTP**
- **Traite avant de normaliser** — ne monte jamais juste l'audio brut
- **Sois constant** sur chaque épisode

---

Tu ne veux pas t'embêter avec des meters et des limiteurs ? [Envoie sur Munchy Cow](/users/log-in) et le loudness est géré automatiquement — avec la chaîne de traitement complète qui fait que ça sonne vraiment bien à ce niveau. 3 heures gratuites, sans carte bancaire.
