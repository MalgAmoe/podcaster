%{
  title: "Comment enregistrer un audio propre dans Audacity",
  description: "Conseils pratiques sur le choix du micro, le réglage du gain, le traitement acoustique et les paramètres d'Audacity pour des enregistrements propres dès le départ.",
  date: ~D[2026-03-05]
}
---

La règle d'or de l'audio est simple : à mauvaise prise de son, mauvais résultat. Aucun traitement ne rattrapera un enregistrement fait avec une mauvaise technique. La bonne nouvelle ? En consacrant 15 minutes à bien te préparer, tu t'épargnes des heures d'édition et tu obtiens un résultat nettement meilleur.

Voici tout ce qu'il faut savoir pour enregistrer un audio propre dans Audacity.

## Choisir le bon micro

Si tu enregistres dans un bureau ou une chambre, **utilise un micro dynamique**. Des modèles comme le Rode PodMic, le Shure SM58 ou le Samson Q2U sont abordables et rejettent naturellement le bruit de fond. Ils sont moins sensibles que les micros à condensateur — et c'est justement ce qu'on veut dans une pièce non traitée.

Les micros à condensateur captent plus de détails, mais aussi chaque clic de clavier, ronronnement de ventilateur et voiture qui passe. Garde-les pour les espaces traités acoustiquement.

Quel que soit ton choix, assure-toi qu'il a un **diagramme polaire cardioïde**. Les micros cardioïdes rejettent le son qui vient des côtés et de l'arrière, ce qui garde le bruit ambiant hors de ton enregistrement.

## Se rapprocher du micro

C'est le conseil qui change tout. Place-toi à **15-30 cm** du micro. Un repère simple : environ quatre doigts de largeur depuis la capsule.

Avec un micro dynamique, tu peux te rapprocher encore (5-8 cm) pour un son plus chaud et intimiste. Mais ne colle pas tes lèvres dessus — ça provoque un excès de graves dû à l'effet de proximité.

**Oriente le micro à environ 30 degrés** par rapport à ta bouche au lieu de parler en face. Ça réduit les plosives (ces sons percutants sur les P, B et T) tout en gardant un son plein et naturel. Un filtre anti-pop aide aussi — procure-t'en un si tu n'en as pas encore.

## Bien configurer Audacity

Avant d'appuyer sur enregistrer, vérifie ces réglages :

**Fréquence d'échantillonnage :** 44 100 Hz est le standard pour la voix. Passe à 48 000 Hz si l'audio accompagne une vidéo. Au-delà, ça n'apporte rien pour de la voix.

**Résolution :** Garde la valeur par défaut d'Audacity à 32-bit float pour l'enregistrement et le montage. Ça te donne énormément de marge et un souffle très bas. Exporte en 16-bit pour le fichier final.

**Canaux :** Enregistre en **1 (Mono)**. La voix est une source mono — le stéréo double le poids du fichier pour rien. Une heure en mono fait environ 310 Mo, contre 620 Mo en stéréo.

## Bien régler le gain

C'est là que la plupart des débutants se plantent. Règle tes niveaux sur le **matériel** — le bouton de gain de ton interface audio ou de ton micro USB — pas dans le logiciel.

Voici comment faire :

1. Dans Audacity, clique sur l'icône du micro dans la barre des vumètres et sélectionne **Start Monitoring**
2. Parle à ton volume normal
3. Ajuste le gain d'entrée pour que les crêtes se situent entre **-12 dBFS et -6 dBFS**
4. Garde de la marge pour les moments où tu ris ou tu t'emballes — les crêtes ne doivent jamais atteindre 0 dBFS

![Agrandis le vumètre d'entrée d'Audacity en tirant son bord](/images/blog/resize-meter.png)

**Astuce :** Tire le bord du vumètre d'entrée pour l'élargir — par défaut il est minuscule et on n'y voit rien.

L'écrêtage numérique est une distorsion agressive et irréparable. Mieux vaut enregistrer un peu trop bas qu'un peu trop fort.

**Un piège classique :** enregistrer trop bas puis gonfler le volume avec Amplify ou Normalize au montage. Ça remonte tout — y compris le souffle. Capture un bon niveau dès le départ.

## Traiter ta pièce (sans se ruiner)

Pas besoin d'un studio pro. Quelques ajustements simples changent tout :

**Options gratuites :**
- Enregistre dans une **petite pièce meublée** — moquette, rideaux, canapé. Un placard rempli de vêtements fait une excellente cabine vocale improvisée
- Accroche des **couvertures épaisses ou des couettes** sur les murs derrière et à côté du micro
- Pose un **tapis sur les sols durs** pour atténuer les réflexions
- Place des **étagères remplies de livres** face au micro — elles diffusent naturellement le son

**Options économiques (25-30 € par panneau) :**
- Fabrique des panneaux acoustiques avec des cadres en bois, de la laine de roche et du tissu
- Place-les aux points de première réflexion : derrière le micro, derrière toi et sur les murs latéraux

**Par ordre de priorité :** d'abord derrière le micro, puis derrière toi, ensuite les côtés, et enfin le plafond.

## Supprimer le bruit avant d'enregistrer

La réduction de bruit au montage est toujours un compromis. Autant éliminer le bruit à la source :

- **Éteins** la clim, les ventilateurs, le chauffage et tout ce qui souffle dans la pièce
- **Ferme fenêtres et portes** — même une fenêtre entrouverte laisse passer la circulation et le vent
- **Coupe** les appareils inutiles — écrans, disques durs externes et tours d'ordinateur sont souvent les premiers coupables
- Si tu dois enregistrer près d'un ordi, oriente l'**arrière du micro** (son point d'atténuation) vers la machine

**Contre les ronflements électriques :**
- Branche tout le matériel sur la **même multiprise** pour éviter les boucles de masse
- Utilise des **câbles XLR symétriques** plutôt que du jack 3,5 mm — ils rejettent les interférences
- Avec un micro USB, branche-le directement sur l'ordinateur — évite les hubs

Applique ces bases et tes enregistrements seront déjà plus propres que la plupart des podcasts — avant même d'ouvrir un plugin.

Et quand tu voudras aller plus loin, [essaie Munchy Cow](/users/log-in). Envoie ton enregistrement et on s'occupe du reste — bruit de fond, réverbération, niveaux inégaux — sans te faire sonner comme un robot.
