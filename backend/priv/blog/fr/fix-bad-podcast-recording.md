%{
  title: "Comment sauver un mauvais enregistrement de podcast",
  description: "Ton épisode sonne terrible. Avant de réenregistrer ou de tout jeter, voici ce qui est vraiment récupérable — et ce qui ne l'est pas.",
  date: ~D[2026-03-08]
}
---

Tu viens de finir d'enregistrer un super épisode. La conversation était parfaite. Tu réécoutes et quelque chose ne va pas. Peut-être que tout ne va pas.

Avant de paniquer, de réenregistrer ou de tout jeter — la plupart des problèmes d'enregistrement sont réparables en post-production. Certains ne le sont pas. Voici comment faire la différence.

## Problèmes que tu peux réparer

### Bruit de fond

Bourdonnement de la clim, ventilateurs d'ordinateur, trafic dehors, bruit du frigo. Tu ne l'as pas remarqué pendant l'enregistrement parce que ton cerveau l'a filtré. Le micro, lui, non.

**À quel point ça peut être mauvais et rester réparable ?** Assez mauvais. La réduction spectrale de bruit moderne peut supprimer le bruit stationnaire (bourdonnements constants, souffles, bruit de ventilateurs) presque complètement sans affecter ta voix. Même un bruit modéré se nettoie bien.

**Outils :** iZotope RX est le standard de l'industrie pour la réparation manuelle. Audacity a une réduction de bruit basique qui fonctionne pour les cas légers. Adobe Podcast Enhance Speech utilise l'IA et gère bien le bruit mais peut sonner traité. Munchy Cow exécute une suppression spectrale du bruit automatisée et calibrée sur chaque fichier.

### Problèmes de volume

Une personne est forte, l'autre est basse. Ou le volume saute parce que quelqu'un s'éloigne du micro.

**La solution :** Compression et normalisation. Un compresseur équilibre la dynamique. Puis la normalisation fixe le niveau global.

Si tu as des pistes séparées pour chaque personne (tu devrais — enregistre toujours en multipiste), tu peux niveler chaque personne indépendamment. Ça donne de bien meilleurs résultats que de réparer une seule piste mixée.

### Clics de bouche et pops

Ces sons de clic humides entre les mots, ou les pops durs des plosives sur P et B. Plus perceptibles au casque, et une fois que tu les entends, tu ne peux plus ne pas les entendre.

**La solution :** Les algorithmes de de-clicking les détectent et les suppriment automatiquement. Pour les plosives, un filtre passe-haut à 80 Hz supprime le coup de basse fréquence sans affecter la voix.

### Sibilance

Sons aigus et perçants de S et CH. Certains micros et certaines voix sont pires que d'autres. Ça cause de la fatigue auditive.

**La solution :** Un de-esser détecte les fréquences sibilantes (généralement 4-8 kHz) et les réduit automatiquement.

### Son terne ou boueux

Ton enregistrement sonne comme si tu parlais à travers une couverture. Généralement parce que tu es trop loin du micro, un micro de basse qualité, ou des réflexions de la pièce qui embrouillent les bas-médiums.

**La solution :** L'EQ correctif peut couper l'accumulation boueuse à 200-400 Hz et ajouter de la présence dans la plage 2-5 kHz. La différence est spectaculaire — c'est comme essuyer la buée d'une vitre.

### Clipping léger

De brefs moments où l'audio a distordu parce que le niveau d'entrée était trop élevé. Si c'est occasionnel — un rire fort ou un cri d'excitation — c'est généralement réparable.

**La solution :** Les algorithmes de de-clipping reconstruisent les crêtes de la forme d'onde qui ont été coupées. Ça fonctionne bien pour des clips brefs.

## Problèmes difficiles à réparer

### Réverb forte de la pièce

C'est le gros morceau. Si ton enregistrement sonne comme si tu étais dans une salle de bain ou une grande pièce vide — c'est extrêmement difficile à supprimer proprement.

**Pourquoi c'est difficile :** La réverb, c'est ta voix mélangée avec des centaines de réflexions de ta voix, toutes à des retards et fréquences légèrement différents. Essayer de la supprimer, c'est comme essayer de retirer la crème du café.

**Ce qui est réaliste :**
- Son de pièce léger (petit bureau, chambre) : La dé-réverb gère bien ça.
- Réverb modérée (grande pièce, sols durs) : Amélioration notable mais pas parfaite.
- Réverb forte (salle de bain carrelée, garage vide) : Aucun logiciel ne corrige ça proprement. Pense à réenregistrer.

**La vraie solution :** Traite ton espace d'enregistrement. Couvertures, tapis, panneaux de mousse, voire enregistrer dans un placard plein de vêtements.

### Clipping sévère

Si tout l'enregistrement est distordu — les niveaux étaient bien trop élevés tout du long — il n'y a rien à faire. Le de-clipping fonctionne sur des pics occasionnels, pas sur de la distorsion soutenue. Les données originales de la forme d'onde sont perdues.

**Prévention :** Enregistre à des pics de -12 à -6 dBFS. Laisse de la marge. Tu peux toujours rendre un audio silencieux plus fort, mais tu ne peux pas dé-détruire un audio clippé.

### Crosstalk et fuite

Quand le micro d'une personne capte la voix de l'autre, tu obtiens un son doublé et déphasé. Aucune solution automatisée fiable n'existe.

**Prévention :** Utilise un casque fermé, garde les micros proches des bouches et éloignés les uns des autres, et enregistre toujours des pistes séparées.

## Le workflow de sauvetage

Si tu as un mauvais enregistrement que tu dois sauver, voici l'ordre des opérations :

1. **Évalue honnêtement.** Écoute les pires 30 secondes. Y a-t-il de la réverb forte ou du clipping soutenu ? Si oui, demande-toi si réenregistrer n'est pas mieux.
2. **Répare ce qui est réparable d'abord.** Suppression du bruit, dé-réverb (si légère), de-clicking — avant tout autre traitement.
3. **Équilibre la dynamique.** Compression et nivellement viennent après le nettoyage.
4. **Sculpte le son.** EQ correctif pour corriger les problèmes de fréquence, puis amélioration pour ajouter de la clarté.
5. **Fixe les niveaux finaux.** Normalisation du loudness et limitation en dernier.

L'ordre compte — l'EQ avant la réduction de bruit rend le bruit plus difficile à supprimer. La compression avant le de-clicking rend les clics plus forts.

Pour un regard plus approfondi sur chaque étape de traitement, lis [Comment rendre ton podcast professionnel](/blog/podcast-audio-cleanup).

## L'option automatisée

Tu peux faire tout ça manuellement dans un DAW avec les bons plugins. Si tu sais ce que tu fais, tu obtiendras de bons résultats. Sinon, tu risques d'empirer les choses — trop de réduction de bruit sonne robotique, un mauvais EQ sonne creux, la sur-compression sonne écrasée.

[Munchy Cow](/) exécute toute cette chaîne automatiquement. Chaque fichier est d'abord analysé — plancher de bruit, caractéristiques de réverb, équilibre spectral, dynamique — puis chaque étape de traitement est calibrée sur ce qui a été trouvé.

3 heures gratuites. Sans carte bancaire. Envoie ton pire enregistrement et regarde ce qui en sort.

## Comment éviter d'avoir besoin d'un sauvetage la prochaine fois

- **Rapproche-toi du micro.** 15-30 cm. La plus grande amélioration que la plupart des gens peuvent faire.
- **Utilise un micro dynamique** si ta pièce n'est pas traitée. Ils rejettent plus le son de la pièce que les micros à condensateur.
- **Enregistre des pistes séparées.** Toujours. Ça te donne infiniment plus de contrôle en post.
- **Monitore avec un casque fermé.** Pour entendre les problèmes au moment où ils arrivent.
- **Vérifie tes niveaux avant de commencer.** Vise des pics autour de -12 dBFS.
- **Fais un enregistrement test de 30 secondes** et réécoute-le avant le vrai. Ça attrape 90% des problèmes.
