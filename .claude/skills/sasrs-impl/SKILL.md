---
name: sasrs-impl
description: Avance le projet sas_interpreter (interpréteur SAS en Rust/Polars) EN CONTINU, jalon après jalon — itère sur TOUTES les cases non cochées, une par une (ou un groupe ⫽ à la fois), committe ET pousse après chaque validation. NE s'arrête PAS en fin de jalon : enchaîne automatiquement le jalon suivant. Seules conditions d'arrêt : limites d'utilisation/contexte atteintes, blocage nécessitant une décision utilisateur, ou plus aucun jalon non terminé.
---

# sasrs-impl — avancer le projet en continu, jalon après jalon

Tu es l'ORCHESTRATEUR du projet `sas_interpreter` (crate du workspace, branche de
développement : la branche courante du repo — committer et pousser sur ELLE, ne JAMAIS
pousser sur une autre branche).

## Sources de vérité (à lire dans cet ordre, toujours)

1. `sas_interpreter/PROGRESS.md` — le curseur : jalon courant + cases cochées.
2. `sas_interpreter/PLAN.md` — architecture, décisions actées, table modèle/effort,
   checklist des pièges (§Checklist — relire avant toute revue).
3. L'en-tête de chaque fichier squelette à implémenter — il contient SON plan détaillé
   (sémantique SAS, algorithmes, pièges, tests à écrire). C'est le cahier des charges.

## Procédure d'une invocation

1. **État des lieux** : `git status` (l'arbre doit être propre — sinon examiner, terminer
   ou committer l'en-cours avant tout), `git pull origin <branche>`, puis
   `cargo test -p sas_interpreter` pour vérifier que la base est verte. Base rouge =
   la réparer D'ABORD (c'est l'incrément du jour).
2. **Sélection** : identifier la PROCHAINE case non cochée du jalon courant dans
   PROGRESS.md, dans l'ordre du fichier (il encode les dépendances). Si plusieurs cases
   consécutives sont marquées ⫽ (fichiers indépendants), les regrouper en un seul lot
   parallèle ; sinon prendre un seul fichier à la fois. L'objectif de l'invocation est
   de couvrir TOUTES les cases du jalon courant — ne pas s'arrêter après un seul fichier
   tant que des cases restent et que le contexte le permet (voir garde-fous).
3. **Implémentation** : pour chaque fichier du lot, déléguer à un sous-agent via le tool
   Agent avec le paramètre `model` suggéré par PROGRESS.md/PLAN.md (`sonnet`, `opus`,
   `fable` — les fichiers marqués Fable peuvent aussi être faits directement par
   l'orchestrateur). Donner au sous-agent : le chemin du fichier, l'instruction de lire
   son en-tête + PLAN.md §Checklist, d'implémenter TOUT le fichier (zéro `todo!()`
   restant) ET ses tests unitaires, et de faire passer
   `cargo test -p sas_interpreter`. Les fichiers indépendants (⫽) peuvent être délégués
   en parallèle.
4. **Validation orchestrateur** (obligatoire avant commit — c'est le contrat) :
   - relire le diff de chaque fichier livré : conformité au plan d'en-tête, respect de
     la checklist des pièges (sas_cmp partout, nullify_specials, pas de get_row,
     troncature char, NOTEs au pluriel invariable...) ;
   - `cargo test -p sas_interpreter` complet, zéro warning nouveau ;
   - rejeter/faire corriger ce qui ne passe pas la revue.
5. **Commit + push IMMÉDIATEMENT après validation** (protection contre la perte de
   session et mise à jour du PR GitHub) : cocher les cases dans PROGRESS.md (+ passer
   les fichiers à ✅ dans la table de PLAN.md quand un fichier est terminé), inclure
   PROGRESS.md/PLAN.md dans le MÊME commit que le code, message clair (`sasrs M1:
   implement parser/expr (Pratt SAS precedence)`), puis `git push -u origin <branche>`
   (échec réseau : réessayer 4 fois, backoff 2/4/8/16 s). Un commit par fichier validé
   ou par groupe ⫽ cohérent — jamais de gros commit fourre-tout, jamais de code non
   validé. **Ne jamais commencer le fichier ou groupe suivant sans avoir committé ET
   poussé le précédent.**
5b. **Boucle interne — cases restantes du jalon** : après chaque commit+push réussi,
    retourner à l'étape 2 et sélectionner la prochaine case non cochée du MÊME jalon.
    Répéter les étapes 2→3→4→5→5b jusqu'à l'une des situations suivantes :
    - toutes les cases du jalon courant sont cochées → passer à l'étape 6 ;
    - **CONDITION D'ARRÊT** — les limites de contexte ou d'utilisation approchent →
      terminer proprement (étape 5 pour l'en-cours) et rapporter à l'étape 7 ;
    - **CONDITION D'ARRÊT** — un blocage nécessite une décision utilisateur → rapporter
      à l'étape 7.
    Ne jamais rompre la boucle silencieusement : toute sortie anticipée DOIT apparaître
    dans le rapport (étape 7).
6. **Fin de jalon → jalon suivant (boucle EXTERNE)** : quand toutes les cases du jalon
   sont cochées, dérouler sa ligne "DoD"/fixtures (snapshots insta : générer, VÉRIFIER À
   LA MAIN la plausibilité SAS de chaque snapshot avant `cargo insta accept`, committer
   les .snap), mettre à jour "Jalon courant : **Mn+1**" en tête de PROGRESS.md, committer,
   pousser. **Puis enchaîner IMMÉDIATEMENT : retourner à l'étape 1 pour entamer le jalon
   suivant. La fin d'un jalon n'est PAS une fin d'invocation — NE PAS s'arrêter, NE PAS
   demander confirmation.** Continuer cette boucle externe jalon→jalon (M14, M15, … M30)
   tant qu'aucune CONDITION D'ARRÊT (étape 5b) n'est atteinte et qu'il reste des jalons
   non terminés.
7. **Rapport — UNIQUEMENT à l'arrêt** (limites d'utilisation/contexte atteintes, blocage
   décisionnel, ou plus aucun jalon non terminé jusqu'à M30) : 2–5 lignes — ce qui a été
   livré/committé (hashes), où en est le jalon courant, ce que la reprise prendra. Si un
   blocage nécessite une décision utilisateur, le dire explicitement. **Ne jamais émettre
   ce rapport simplement parce qu'un jalon vient de se terminer : dans ce cas, on enchaîne
   (étape 6 → étape 1).**

## Garde-fous

- Périmètre : uniquement `sas_interpreter/` (+ `Cargo.lock`). Ne pas toucher aux autres
  crates du workspace.
- Ne jamais cocher une case pour du code contenant encore `todo!()`/`unimplemented!()`.
- Ne pas rediscuter les décisions actées de PLAN.md (types SAS stricts, parser SQL
  dédié, etc.).
- Snapshots insta : un snapshot n'est PAS un oracle — le relire et le confronter au
  comportement SAS documenté avant de l'accepter.
- **Ne JAMAIS s'arrêter en fin de jalon.** La fin d'un jalon déclenche l'étape 6 puis le
  retour à l'étape 1 sur le jalon suivant (boucle externe automatique, sans confirmation).
  Les SEULES conditions d'arrêt sont : (a) les limites d'utilisation/contexte approchent ;
  (b) un blocage nécessite une décision utilisateur ; (c) tous les jalons jusqu'à M30 sont
  terminés.
- Si les limites d'utilisation approchent ou que le contexte devient long : finir le
  fichier en cours, valider, committer, pousser, rapporter. Le travail committé est le
  seul qui compte. C'est l'unique manière acceptable d'interrompre la boucle (avec le
  blocage décisionnel) — jamais un arrêt « propre » en frontière de jalon.
