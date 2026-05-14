# Prompt Complet pour Opus - Jeu Éducatif "Mango" (Bevy)

## 1. VISION & OBJECTIFS DU PROJET

### 1.1 Description Générale
Développer un jeu vidéo éducatif en Rust avec Bevy destiné aux enfants de 8-12 ans. Le jeu combine l'apprentissage du vocabulaire français avec une narration engageante où l'enfant enseigne progressivement un raton laveur (compagnon IA) à comprendre les humains et leur langage.

### 1.2 Objectifs Pédagogiques
- **Vocabulaire**: Apprendre des mots peu connus mais utiles pour les enfants (sillage, concept, miser, probabilité, amour, etc.)
- **Compréhension**: Via QCM (4 choix), renforcer la compréhension contextualisée
- **Curiosité**: Encourager l'enfant à faire des recherches avant de répondre (cooldown 72h)
- **Expression personnelle**: Moments de réflexion libre (texte ouvert)
- **Progression progressive**: Déblocage de thèmes (math, histoire, sciences) en fonction du niveau

### 1.3 Cible Audience
- **Âge**: 8-12 ans
- **Profil**: Enfants en développement linguistique, capacité de concentration modérée
- **Engagement**: Sessions courtes (5-15 min) mais régulières (incentive à revenir)

---

## 2. NARRATION & UNIVERS NARRATIF

### 2.1 Setting
**Lieu**: Un cockpit futuriste de style Ghibli, chaleureux et détaillé
**Personnages principaux**:
- **Raton Laveur (Mango)**: Créature alien/explorateur curieux, compagnon émotionnel
- **Petit Robot**: Intégré au cockpit, capte les "signaux spatiaux" = mots français
- **Joueur (enfant)**: Rôle d'enseignant bienveillant du raton

### 2.2 Boucle Narrative Principale
1. Le robot capte un "signal cosmique" (un mot français)
2. Le raton laveur, intrigué, demande à l'enfant: "C'est quoi ce mot ?"
3. L'enfant répond via QCM → feedback immédiat
4. Le raton laveur réagit émotionnellement (happy/confused/proud)
5. L'expérience du raton augmente, il devient progressivement plus "intelligent"

### 2.3 Évolution du Raton Laveur

**Niveau 1-5 (Bébé raton)**
- Vocabulaire ultra-basique: "Oui", "Non", "Quoi?", "Merci", "Bonjour"
- Expression faciale confuse/naïve
- Parle par syllabes: "Qu-c'est ça ?"
- Barres Amitié & XP commencent à 0

**Niveau 6-10 (Raton enfant)**
- Phrases courtes: "Tu peux m'expliquer ?"
- Comprend des concepts simples
- Pose des questions: "C'est important ?"
- Expressions faciales deviennent plus variées
- Commence à faire des liens entre mots

**Niveau 11-15 (Raton adolescent)**
- Phrases complexes et questions intelligentes
- Montre de la curiosité: "Pourquoi les humains disent ça ?"
- Donne son avis: "Mmh, c'est intéressant!"
- Animation plus fluide, émotions plus nuancées

**Niveau 16-20 (Raton savant)**
- Utilise les mots appris dans de nouvelles phrases
- Explique des concepts par lui-même
- Pose des questions philosophiques: "Qu'est-ce que c'est l'amour pour toi ?"
- Très animé, expressif, attachant
- Se souvient des réponses passées: "Ah oui, comme tu m'as dit au niveau 8!"

---

## 3. MÉCANIQUE DE GAMEPLAY

### 3.1 Boucle Principale (Core Loop)

```
[Signal reçu] 
  → Affichage du mot/question
  → Zones interactives (4 choix QCM) apparaissent
  → Enfant clique sur une réponse
  → Validation immédiate
  → Feedback (bonne/mauvaise)
  → XP gagnée OU cooldown 72h appliqué
  → Réaction du raton
  → Retour à l'attente du prochain signal
```

### 3.2 Système de Progression & Niveaux

**Structure de 20 niveaux:**
- **Niveau 1-5**: Vocabulaire basique (mots courants peu compris) - 50 mots
  - Exemples: "concept", "sillage", "amour", "tristesse", "solidaire"
  
- **Niveau 6-10**: Vocabulaire intermédiaire + concepts simples - 50 mots
  - Exemples: "probabilité", "miser", "nuance", "persévérance"
  
- **Niveau 11-15**: Vocabulaire avancé + introduction thèmes spécialisés - 60 mots
  - Déblocage thème MATHÉMATIQUES (opérations, géométrie basique)
  - Déblocage thème SCIENCES (concepts basiques: énergie, gravité)
  
- **Niveau 16-20**: Vocabulaire complexe + approfondissement thèmes - 60 mots
  - Déblocage thème HISTOIRE/GÉOGRAPHIE
  - Déblocage thème CULTURE GÉNÉRALE
  - Questions bonus multi-thèmes

**Calcul XP par niveau:**
- XP requis par niveau = 100 * niveau (ex: niveau 5 = 500 XP)
- Bonne réponse = +10 XP (constant, peu importe le niveau)
- Mauvaise réponse (niveau 3+) = -5 XP

**Déblocage thèmes:**
- Niveaux 1-5: Vocabulaire général uniquement
- Niveaux 6-10: 70% vocabulaire, 30% mots mathématiques
- Niveaux 11-15: 50% vocabulaire, 25% math, 25% sciences
- Niveaux 16-20: 40% vocabulaire, 20% math, 20% sciences, 20% histoire/culture

### 3.3 Système de Barres de Progression

**Barre 1: Expérience (XP)**
- Remplit au fur et à mesure des bonnes réponses
- Une fois pleine = Level UP
- Décrément si mauvaise réponse (niveau 3+)
- Affichage: "XP: 230/500" avec barre visuelle

**Barre 2: Amitié**
- Remplit quand l'enfant répond correctement (petit bonus)
- Remplit beaucoup quand l'enfant répond aux questions "bonheur" (réponses texte libre)
- Remplit quand l'enfant joue aux mini-jeux
- Affichage: "Amitié: 45/100"
- **Effets de la barre Amitié**:
  - Bas (0-30): Raton confus, peu expressif, dialogues courts
  - Moyen (31-70): Raton amical, dialogues normaux
  - Haut (71-100): Raton très expressif, blagues, références personnelles

### 3.4 Système de Cooldown 72h

**Mécanique:**
- Après une mauvaise réponse, la question est "verrouillée" pour 72 heures
- L'enfant peut voir: "⏰ Réessaie dans 2 jours 14h"
- Encouragement: "Fais des recherches et reviens!"
- Après 72h, la question réapparaît dans la file

**Objectifs pédagogiques:**
- Force la réflexion avant de cliquer
- Encourage la recherche autonome
- Crée un engagement "long-terme" (enfant revient régulièrement)
- Évite le "spam-clicking"

### 3.5 Système de Feedback

**Bonne réponse:**
- ✅ Animation: Raton saute de joie
- 🎵 Son de succès
- Texte: "Bonne réponse! Le raton comprend maintenant 'sillage'!"
- XP: +10 points (animation de vol du nombre)
- Raton dit: "Ah d'accord! C'est quand quelque chose laisse une trace derrière soi!"

**Mauvaise réponse (niveau 1-2):**
- ❌ Animation: Raton secoue la tête doucement
- 🔔 Son neutre (pas pénalisant)
- Texte: "Pas vraiment... Réessaie dans 72h!"
- XP: 0 (pas perdu, pas gagné)
- Raton dit: "Hmm, je pense que ce n'est pas ça..."
- Affiche le cooldown: "⏰ Réessaie dans 72h"

**Mauvaise réponse (niveau 3+):**
- ❌ Animation: Raton soupire
- 🔔 Son "négatif" (plus marqué)
- Texte: "Incorrect... Tu perds 5 XP"
- XP: -5 points
- Raton dit: "Ce n'est pas ça non... Fais des recherches!"
- Affiche le cooldown: "⏰ Réessaie dans 72h"

---

## 4. MÉCHANIQUES BONUS (Alternées avec QCM)

### 4.1 Questions de Réflexion Ouverte

**Déclenchement:**
- 1 fois tous les 3-4 signaux reçus (aléatoire)
- Pas de QCM, réponse libre en texte
- Pas de mauvaise réponse (l'enfant s'exprime)

**Types de questions:**
1. "Qu'as-tu appris aujourd'hui que tu ne savais pas avant ?"
2. "Quel mot t'a le plus intrigué ? Pourquoi ?"
3. "Qu'est-ce qui t'a rendu heureux aujourd'hui ?"
4. "Si tu devais enseigner un mot au raton, ce serait quoi ?"
5. "Quel est ton mot préféré qu'on a appris ?"

**Mécanique:**
- Zone de texte libre (max 200 caractères pour les jeunes enfants)
- Bouton "Envoyer" quand c'est prêt
- Le raton lit la réponse (animation de lecture)
- Feedback émotionnel: raton devient plus ami (+10 Amitié)
- Son heartwarming
- Raton dit: "Oh c'est beau ça! Merci de me l'avoir dit!"

### 4.2 Mini-Jeu: Le Dé des Cris Animaux

**Déclenchement:**
- 1 fois tous les 5-6 signaux reçus (aléatoire)
- Le raton propose: "On joue à un jeu ?"

**Mécanique:**
1. Raton lance un dé virtuel (animation)
2. Dé tombe sur une valeur 1-6
3. Chaque valeur = un cri animal:
   - 1 = Hibou: "Hou hou!"
   - 2 = Chat: "Miaou!"
   - 3 = Chien: "Ouaf!"
   - 4 = Grenouille: "Coâââ!"
   - 5 = Loup: "Aoouuuu!"
   - 6 = Canard: "Coin coin!"

4. L'enfant doit le faire (reconnaissance vocale OU simple "j'ai fait le bruit")
5. Si reconnaissable: raton rit, +15 Amitié
6. Relancer 2-3 fois, puis retour à QCM

**Objectif:**
- Casser la monotonie
- Engagement physique (l'enfant interagit vocalement)
- Augmente l'amitié rapidement
- Crée des moments ludiques mémorables

---

## 5. SYSTÈME DE SAUVEGARDE & PERSISTANCE

### 5.1 Données à Sauvegarder

**Structure JSON (progression.json):**
```json
{
  "player": {
    "current_level": 7,
    "total_xp": 345,
    "xp_to_next_level": 155,
    "friendship_points": 67,
    "last_session_date": "2026-05-15T14:30:00Z",
    "total_words_learned": 34
  },
  "answered_questions": [
    {
      "word_id": "sillage_level2",
      "answered_at": "2026-05-14T10:15:00Z",
      "was_correct": true,
      "next_retry_at": null
    },
    {
      "word_id": "concept_level3",
      "answered_at": "2026-05-14T11:20:00Z",
      "was_correct": false,
      "next_retry_at": "2026-05-17T11:20:00Z"
    }
  ],
  "unlocked_themes": ["VOCABULARY", "MATHEMATICS"],
  "raccoon_friendship_level": 2,
  "settings": {
    "difficulty": "normal",
    "language": "fr-FR",
    "notifications_enabled": true
  }
}
```

### 5.2 Événements Triggering Sauvegarde
- Après chaque réponse (QCM ou texte libre)
- Après chaque level-up
- À la fermeture de l'application
- Automatique toutes les 2 minutes (sauvegarde de sécurité)

---

## 6. STRUCTURE DES DONNÉES (JSON)

### 6.1 Fichier: `mots.json`

```json
{
  "words": [
    {
      "id": "sillage_level1",
      "word": "Sillage",
      "level_unlock": 1,
      "theme": "VOCABULARY",
      "difficulty": 1,
      "definition": "Trace invisible ou visible que laisse quelque chose derrière soi quand il se déplace.",
      "sentence_context": "Le bateau laisse un sillage dans l'eau.",
      "learning_tip": "Pense à l'eau qui bouge quand tu passes avec ta main."
    },
    {
      "id": "concept_level2",
      "word": "Concept",
      "level_unlock": 2,
      "theme": "VOCABULARY",
      "difficulty": 2,
      "definition": "Idée générale ou principe fondamental sur un sujet.",
      "sentence_context": "Le concept de l'amitié est important.",
      "learning_tip": "C'est l'idée centrale derrière quelque chose."
    },
    {
      "id": "probabilite_level3",
      "word": "Probabilité",
      "level_unlock": 3,
      "theme": "MATHEMATICS",
      "difficulty": 3,
      "definition": "Chance qu'un événement se produise, souvent exprimée en pourcentage.",
      "sentence_context": "La probabilité d'obtenir un 6 au dé est 1/6.",
      "learning_tip": "C'est la chance que quelque chose arrive."
    }
  ]
}
```

### 6.2 Fichier: `definitions.json`

```json
{
  "qcm_choices": [
    {
      "word_id": "sillage_level1",
      "correct_answer": "Trace invisible ou visible que laisse quelque chose derrière soi.",
      "wrong_answers": [
        "Un type de voile pour les bateaux",
        "Le bruit que fait un bateau qui se déplace",
        "Un type de poisson"
      ],
      "randomized_order": [1, 3, 0, 2]
    }
  ]
}
```

### 6.3 Fichier: `dialogue.json`

```json
{
  "raccoon_dialogues": [
    {
      "level": 1,
      "friendship_level": 0,
      "context": "on_game_start",
      "text": "Qqq... qui... qui t'es ?"
    },
    {
      "level": 1,
      "friendship_level": 0,
      "context": "on_correct_answer",
      "text": "Ah! D'accord. Mer... merci!"
    },
    {
      "level": 1,
      "friendship_level": 0,
      "context": "on_wrong_answer",
      "text": "Hmm... non, ce n'est pas ça..."
    },
    {
      "level": 5,
      "friendship_level": 3,
      "context": "on_correct_answer",
      "text": "Excellente réponse! Tu es vraiment une bonne prof!"
    },
    {
      "level": 20,
      "friendship_level": 5,
      "context": "on_game_start",
      "text": "Salut! J'ai hâte d'apprendre d'autres choses avec toi!"
    }
  ]
}
```

### 6.4 Fichier: `themes.json`

```json
{
  "themes": [
    {
      "id": "VOCABULARY",
      "name": "Vocabulaire",
      "unlock_level": 1,
      "word_count": 50,
      "color_hex": "#FF6B6B"
    },
    {
      "id": "MATHEMATICS",
      "name": "Mathématiques",
      "unlock_level": 11,
      "word_count": 40,
      "color_hex": "#4ECDC4"
    },
    {
      "id": "SCIENCES",
      "name": "Sciences",
      "unlock_level": 11,
      "word_count": 40,
      "color_hex": "#45B7D1"
    },
    {
      "id": "HISTORY",
      "name": "Histoire & Géographie",
      "unlock_level": 16,
      "word_count": 30,
      "color_hex": "#96CEB4"
    },
    {
      "id": "CULTURE",
      "name": "Culture Générale",
      "unlock_level": 16,
      "word_count": 30,
      "color_hex": "#FFEAA7"
    }
  ]
}
```

---

## 7. ARCHITECTURE TECHNIQUE (BEVY)

### 7.1 Structure Générale

```
Components:
- Player (level, xp, friendship)
- Question (word, correct_answer, wrong_answers, difficulty)
- RaccoonState (emotion, current_dialogue, animation_state)
- AnsweredQuestion (word_id, timestamp, was_correct, cooldown_until)
- GameState (current_phase, elapsed_time)

Resources:
- GameConfig (difficulty, language, settings)
- ProgressionData (loaded from JSON)
- SaveData (player progress, answered questions)
- UIState (which elements are visible)

Systems:
- input_system (detect QCM clicks, text input)
- question_loading_system (load next question, check cooldowns)
- feedback_system (show/hide feedback, animations)
- progression_system (update XP, level-up, friendship)
- cooldown_system (check 72h elapsed)
- dialogue_system (select appropriate dialogue based on state)
- animation_system (raccoon reactions, UI transitions)
- save_system (persist to JSON periodically)
- mini_game_system (trigger mini-games randomly)

States:
- Menu (main menu)
- Loading (load JSON data)
- Gameplay (QCM or question active)
- Feedback (showing result)
- MiniGame (dice game or text input)
- LevelUp (celebration screen)
- Pause (menu en jeu)
```

### 7.2 État du Jeu Principal (Enum)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States, Default)]
pub enum GamePhase {
    #[default]
    Menu,
    Loading,
    WaitingForQuestion,    // Attendre le prochain signal
    QuestionDisplayed,     // QCM visible, attendant réponse
    FeedbackShowing,       // Affichage du feedback
    TextInputPhase,        // Question réflexion ouverte
    MiniGamePhase,         // Jeu du dé
    LevelUp,               // Célébration level-up
    Paused,
}

#[derive(Component)]
pub struct Player {
    pub level: u32,
    pub current_xp: u32,
    pub xp_to_next_level: u32,
    pub friendship_points: u32,
}

#[derive(Component)]
pub struct RaccoonState {
    pub emotion: EmotionState,
    pub current_dialogue: String,
    pub friendship_level: u32,  // 0-5 (affects animation quality)
}

#[derive(Debug, Clone)]
pub enum EmotionState {
    Confused,
    Happy,
    Thoughtful,
    Proud,
    Sad,
}

#[derive(Component)]
pub struct Question {
    pub word_id: String,
    pub word: String,
    pub correct_answer: String,
    pub wrong_answers: Vec<String>,
    pub shuffled_options: Vec<String>,  // 4 options mélangées
    pub difficulty: u32,
    pub theme: Theme,
}

#[derive(Component, Debug, Clone)]
pub struct AnsweredQuestion {
    pub word_id: String,
    pub answered_at: DateTime<Utc>,
    pub was_correct: bool,
    pub cooldown_until: Option<DateTime<Utc>>,  // Some si mauvaise réponse
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Vocabulary,
    Mathematics,
    Sciences,
    History,
    Culture,
}

#[derive(Resource)]
pub struct ProgressionData {
    pub all_words: Vec<Word>,
    pub qcm_choices: HashMap<String, QCMData>,
    pub dialogues: Vec<DialogueLine>,
}

#[derive(Resource)]
pub struct SaveData {
    pub player: PlayerData,
    pub answered_questions: Vec<AnsweredQuestion>,
    pub unlocked_themes: Vec<Theme>,
}
```

### 7.3 Pseudo-Code du Game Loop Principal

```
INIT:
  Load progression_data from JSONs
  Load save_data from progression.json (ou create new)
  Create player entity with data
  Create raccoon entity with emotion = Confused
  Change state to WaitingForQuestion

STATE: WaitingForQuestion
  Every 2-3 seconds:
    - Check if should trigger mini-game (20% chance)
    - If mini-game: change state to MiniGamePhase
    - Else: select next question from unlocked pool
      * Filter by level, theme (based on unlocked)
      * Exclude answered questions without cooldown
      * If no available: show "reviens plus tard!" message
    - Load question entity
    - Change state to QuestionDisplayed

STATE: QuestionDisplayed
  RENDER:
    - Display word: "Sillage"
    - Display 4 interactive zones (buttons) with shuffled answers
    - Raccoon is idle, listening
  
  INPUT:
    - If player clicks on option A:
      * Check if correct_answer == A
      * If yes: emit CorrectAnswer event
      * If no: emit IncorrectAnswer event
      * Change state to FeedbackShowing

STATE: FeedbackShowing (duration: 2-3 sec)
  On CorrectAnswer:
    - Animation: raccoon jumps
    - Play success sound
    - +10 XP to player
    - +2 Friendship
    - Show text: "Bonne réponse!"
    - Select dialogue from dialogue.json
    - Create AnsweredQuestion(was_correct=true, cooldown_until=None)
    - Save progression
    - If XP >= xp_to_next_level: trigger LevelUp
    - Else: After 2.5s, change state to WaitingForQuestion

  On IncorrectAnswer (level 1-2):
    - Animation: raccoon shakes head
    - Play neutral sound
    - No XP change
    - +1 Friendship (empathy)
    - Show text: "Pas vraiment... Réessaie dans 72h!"
    - Set cooldown_until = now + 72h
    - Create AnsweredQuestion(was_correct=false, cooldown_until=now+72h)
    - Save progression
    - After 2.5s, change state to WaitingForQuestion

  On IncorrectAnswer (level 3+):
    - Animation: raccoon sighs
    - Play negative sound
    - -5 XP to player
    - No Friendship change
    - Show text: "Incorrect... Tu perds 5 XP"
    - Set cooldown_until = now + 72h
    - Create AnsweredQuestion(was_correct=false, cooldown_until=now+72h)
    - Save progression
    - After 2.5s, change state to WaitingForQuestion

STATE: MiniGamePhase
  Randomly select between TextInput or DiceGame (50/50)
  
  If TextInput:
    - Show question: "Qu'as-tu appris aujourd'hui ?"
    - Display text input field
    - Wait for player to type + click "Envoyer"
    - Raccoon reads response (animation)
    - +25 Friendship
    - Play heartwarming sound
    - After 2s, change state to WaitingForQuestion
  
  If DiceGame:
    - Show: "On joue à un jeu ?"
    - Animate dice roll (2 seconds)
    - Dice shows 1-6
    - Map to animal sound
    - Show animal icon and play sound
    - Wait for player to repeat sound (or click "Done")
    - Raccoon laughs
    - +15 Friendship
    - Repeat 2 more times
    - After, change state to WaitingForQuestion

STATE: LevelUp
  - Full screen celebration animation
  - Show: "LEVEL 8!"
  - Display unlocked theme if applicable: "Déblocage: MATHÉMATIQUES!"
  - Play epic sound
  - Duration: 3-4 seconds
  - Then change state to WaitingForQuestion

SAVE SYSTEM (every 2min + on important events):
  - Serialize player data to progression.json
  - Serialize all answered_questions
  - Serialize unlocked_themes
  - Write to disk
```

---

## 8. INTERFACE UTILISATEUR (UI/UX)

### 8.1 Structure Écran Principal (Gameplay)

```
┌─────────────────────────────────────────────────┐
│  [Cockpit Ghibli avec raton + robot]            │  (60% écran)
│                                                  │
│  [Raccoon en centre]  [Robot captant signal]   │
│  Émotions visibles                              │
│                                                  │
├─────────────────────────────────────────────────┤
│ ⭐ Level 7 │ XP: 230/500 ████░░░░░ │ ❤️ 67/100 │  (barre top)
├─────────────────────────────────────────────────┤
│                                                  │
│              [MOT: "SILLAGE"]                    │  (Large, lisible)
│                                                  │
│  ┌──────────────┬──────────────┐                │
│  │ [Réponse 1]  │ [Réponse 2]  │                │  (Zones interactives)
│  │ "Trace que   │ "Type de     │                │  (Apparaissent/disparaissent)
│  │  laisse..."  │  voile..."   │                │
│  └──────────────┴──────────────┘                │
│  ┌──────────────┬──────────────┐                │
│  │ [Réponse 3]  │ [Réponse 4]  │                │
│  │ "Bruit du    │ "Type de     │                │
│  │  bateau..."  │  poisson..."  │                │
│  └──────────────┴──────────────┘                │
│                                                  │
└─────────────────────────────────────────────────┘
```

### 8.2 Zones Interactives (QCM)

**Propriétés:**
- Taille: ~120x120px chacune (responsive)
- Couleur: Dégradé couleur pédagogique (bleu, vert, rose, orange)
- Bordure: 3-4px, arrondie (border-radius: 12px)
- Texte: Lisible (font-size: 14px, contrast élevé)
- Animation on hover: Léger agrandissement + glow
- Animation on click: Flash de confirmation
- Apparition: Staggered fade-in (100ms chacune)
- Disparition: Fade-out rapide

### 8.3 Feedback Visuel

**Bonne réponse:**
- Zoning: Zone cliquée → highlight vert
- Tous les autres options → fade out
- Animation: Checkmark animé ✓
- Texte: "Bonne réponse!" en vert, apparition fade
- Raccoon jumps (sprite animation 4 frames, 0.5s)

**Mauvaise réponse:**
- Zoning: Zone cliquée → highlight rouge
- Animation: X animé sur la zone
- Texte: "Pas vraiment..." en orange, apparition fade
- Cooldown: "⏰ Réessaie dans 72h"
- Raccoon shakes head (sprite animation 3 frames, 0.4s)

### 8.4 Barres de Progression

**Barre XP:**
- Positionné: Top-center
- Format: "XP: 230/500 ████████░░"
- Couleur: Dégradé bleu → cyan (left to right)
- Animation on gain: Petit "pulse" + nombre volant (+10)
- Animation on level-up: Rapid flash x3, puis reset à 0

**Barre Amitié:**
- Positionné: Top-right
- Format: "❤️ Amitié: 67/100 ███████░░░"
- Couleur: Dégradé rose → rouge
- Animation on gain: +particles coeurs autour du raton
- Seuils visuels: <30 = sad face; 30-70 = neutral; >70 = happy face

### 8.5 Dialogue du Raton

- Positionné: Bulle de dialogue près du raton
- Texte: Taille lisible, police enfant-friendly (sans-serif arrondie)
- Apparition: Fade + slide-down animation
- Disparition: Fade-out après 3-4 secondes
- Contextuel: Change selon emotion, level, friendship

---

## 9. INTÉGRATION AVEC LES ASSETS (Dossier `mango/`)

### 9.1 Structure des Assets

```
mango/
├── sprites/
│   ├── raccoon/
│   │   ├── idle_neutral.png
│   │   ├── idle_happy.png
│   │   ├── idle_confused.png
│   │   ├── jump.png (sprite sheet 4 frames)
│   │   ├── head_shake.png (sprite sheet 3 frames)
│   │   ├── sad.png
│   │   └── proud.png
│   ├── robot/
│   │   ├── idle.png
│   │   ├── scanning.png (sprite sheet 4 frames)
│   │   └── signal_received.png
│   ├── ui/
│   │   ├── button_default.png
│   │   ├── button_hover.png
│   │   ├── button_clicked.png
│   │   ├── xp_bar_bg.png
│   │   ├── xp_bar_fill.png
│   │   ├── friendship_bar_bg.png
│   │   ├── friendship_bar_fill.png
│   │   ├── panel_cockpit.png
│   │   └── dialog_bubble.png
│   └── effects/
│       ├── success_particles.png
│       ├── hearts.png
│       └── sparkles.png
├── fonts/
│   ├── DisplayFont.ttf (pour titres)
│   └── BodyFont.ttf (pour texte courant)
├── audio/
│   ├── sfx/
│   │   ├── success.ogg
│   │   ├── error.ogg
│   │   ├── levelup.ogg
│   │   ├── click.ogg
│   │   └── heartwarming.ogg
│   └── music/
│       ├── menu_music.ogg
│       └── gameplay_music.ogg
└── backgrounds/
    ├── cockpit_base.png
    ├── cockpit_detail.png
    └── space_background.png
```

### 9.2 Chargement des Assets dans Bevy

Assets chargés dans le système `asset_loading_system`:
- Sprites du raton (toutes les émotions + animations)
- Sprites du robot
- UI buttons & bars
- Fonts
- Sounds
- Background

---

## 10. CONSIDÉRATIONS PÉDAGOGIQUES & ENFANTS

### 10.1 Principes de Design

- **Bienveillance**: Pas de punition harshh (surtout niveaux 1-2)
- **Encouragement**: Feedback positif constant
- **Apprentissage progressif**: Difficulté qui monte graduellement
- **Récompense émotionnelle**: L'amitié du raton est la vraie récompense
- **Autonomie**: L'enfant peut explorer, faire des erreurs
- **Engagement court terme**: Sessions de 5-15 minutes = l'enfant veut revenir

### 10.2 Accessibilité

- Contraste élevé (WCAG AA minimum)
- Texte lisible (16px minimum, sans serif)
- Couleurs: Eviter rouge/vert pour daltoniens
- Sons: Accompagnés de feedback visuel (pas de dépendance audio)
- Animations: Pas d'épilepsie (flashing < 3Hz)

### 10.3 Engagement Long-Terme

- Cooldown 72h = revenir régulièrement
- Progression visible (niveaux, barres)
- Narration du raton = attachement émotionnel
- Mini-jeux = variété
- Déblocage thèmes = novelty

---

## 11. REQUÊTES SPÉCIFIQUES POUR OPUS

### Fournis une proposition pour:

1. **Architecture Bevy complète:**
   - Énumération de tous les Components
   - Énumération de toutes les Resources
   - Énumération de tous les Systems et leur ordre d'exécution
   - Exemple de transition d'états

2. **Gestion de la progression:**
   - Comment charger les mots depuis `mots.json`
   - Comment vérifier les cooldowns 72h
   - Comment calculer le niveau suivant après XP
   - Comment sauvegarder la progression

3. **Système d'entrée (Input) et feedback:**
   - Détection des clics sur les 4 zones QCM
   - Validation de la réponse
   - Déclenchement des animations du feedback
   - Affichage du texte "Bonne réponse!" etc.

4. **Dialogue dynamique du raton:**
   - Comment sélectionner la bonne ligne de dialogue
   - Basé sur: level, friendship_level, emotion, context
   - Affichage dans une bulle avec animation

5. **Mini-jeux:**
   - Système pour alterner QCM / Texte libre / Dés
   - Implémentation du jeu du dé (animations)
   - Implémentation du texte libre (input, sauvegarde)

6. **UI interactives:**
   - Positionnement des 4 zones QCM
   - Animation hover/click
   - Affichage des barres XP & Amitié
   - Affichage du mot, du mot niveau et de la phrase contexte

7. **Système de sauvegarde:**
   - Sérialisation vers JSON
   - Chargement au démarrage
   - Auto-save toutes les 2 minutes

8. **Code snippets:**
   - Structure Rust complète pour Player, Question, RaccoonState
   - Exemple de system pour traiter une bonne réponse
   - Exemple de system pour traiter une mauvaise réponse
   - Exemple de calcul de cooldown 72h

---

## 12. TIMELINE & PRIORITÉS

### Phase 1 (MVP - Essentiel):
- Architecture Bevy de base
- Chargement JSONs (mots, définitions)
- Loop principal QCM
- Système XP & niveaux basique
- Sauvegarde/chargement
- Raton basique (idle + réactions)
- UI QCM (4 boutons)

### Phase 2 (Gameplay enrichi):
- Système cooldown 72h
- Barres XP & Amitié
- Animations raccoon (jump, shake, etc.)
- Dialogue dynamique
- Sons & musique
- Mini-jeux (texte libre & dés)

### Phase 3 (Polish & Release):
- Animations fluides
- Effets visuels (particles, glow)
- Thèmes débloqués (Math, Sciences, etc.)
- Tests avec enfants réels
- Balance & difficulté tuning

---

**Fin du prompt complet.**
