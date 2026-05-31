# bevy_neural_network

Visualisation interactive d'un réseau de neurones (perceptron multicouche) **en
cours d'apprentissage**, écrite entièrement en [Bevy](https://bevyengine.org/).

L'application matérialise la descente de gradient **pas à pas** :

- une onde lumineuse traverse les couches pendant la **propagation avant** ;
- l'onde des gradients **remonte** pendant la rétropropagation ;
- les **poids** changent (couleur et opacité des arêtes) lors de la mise à jour ;
- la **frontière de décision** (ou la courbe de régression) se forme dans le
  panneau de droite, pendant que la **courbe de loss** décroît en bas.

Le réseau et les datasets sont implémentés à la main, sans bibliothèque de ML.

## Lancer

```bash
cargo run -p bevy_neural_network --release
```

## Contrôles

| Touche | Action |
| --- | --- |
| `P` | Lecture / pause de l'entraînement |
| `Espace` | Avancer d'une phase (forward → backward → mise à jour) |
| `F` (maintenir) | Entraînement rapide sans animation (turbo) |
| `R` | Réinitialiser les poids et la loss |
| `←` / `→` | Sélectionner la couche cachée à éditer |
| `↑` / `↓` | Ajouter / retirer un neurone dans la couche sélectionnée |
| `N` / `M` | Ajouter / retirer une couche cachée |
| `1` `2` `3` `4` | Dataset : XOR · Spirales · Cercles · Régression |
| `+` / `-` | Vitesse de l'animation |
| `[` / `]` | Taux d'apprentissage |

## Datasets

- **XOR** — le classique non linéairement séparable (2 entrées, 1 sortie).
- **Spirales** — deux spirales entrelacées, frontière complexe.
- **Cercles** — disque intérieur contre anneau extérieur.
- **Régression** — approximation de `y = 0.7·sin(π·x)` (1 entrée, 1 sortie).

## Lecture des couleurs

- **Neurones** : bleu = activation négative, sombre = zéro, orange = positive.
- **Arêtes** : bleu = poids négatif, orange = positif, opacité ∝ |poids|.
- **Impulsions** : cyan en propagation avant, orange en rétropropagation.
- **Frontière de décision** : teinte mélangée selon la sortie du réseau.

## Architecture du code

| Fichier | Rôle |
| --- | --- |
| `network.rs` | Le MLP : forward, backward, descente de gradient. |
| `dataset.rs` | Génération des jeux de données jouets. |
| `training.rs` | Ressources d'état et machine à phases pas-à-pas. |
| `layout.rs` | Positions et rayon des neurones. |
| `visualization.rs` | Rendu : neurones, arêtes, impulsions, frontière, loss. |
| `hud.rs` | Affichage texte (état + contrôles). |
| `input.rs` | Gestion du clavier. |
