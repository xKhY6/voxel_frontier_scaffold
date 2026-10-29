# Tests

## Commandes

- `cargo test` : lance tous les tests.
- `cargo clippy --all-targets -- -D warnings` : vérifie tous les targets sans warning.
- `cargo fmt --all -- --check` : vérifie le format (configuration dans `rustfmt.toml`).
- Sous Windows, `lanceur.bat` propose ces contrôles via « Validation complete » et le test de démarrage sans fenêtre via « Verifier le demarrage ».

## Tests existants

- `tests/smoke_test.rs` (`smoke`) : lance le binaire avec `--check` et vérifie qu'il sort avec succès en affichant « Voxel Frontier bootstrap », sans ouvrir de fenêtre.

## Stratégie

- Tests unitaires : logique pure (voxel, collisions, caméra, génération seedée), à côté du code.
- Génération : types de surface par biome, diversité déterministe des biomes, raccords de terrain et arbres traversant les frontières de chunks.
- IA/combat : transitions d'état, gravité/collisions, contournement de murs, montée de marches, arrêt sans chemin, cadence des attaques joueur/ennemi, ciblage mêlée/tir, dégâts d'armes, chargeurs et rechargement.
- Éclairage : propagation, blocage par les solides, continuité entre couches verticales, cycle jour/nuit, direction solaire et temps invalides ; les maillages touchés par un changement d'ombre sont identifiés ; normales des faces présentes dans les sommets.
- Pause : suspension de la simulation et du cycle lumineux, remise à zéro des entrées au changement d'état.
- Fin de partie : blocage de la simulation après victoire et réinitialisation de la partie, de l'inventaire, des ennemis, de l'éclairage et du cache sur R.
- Menus : simulation suspendue à l'accueil, pause et fabrication ; options accessibles depuis la pause, sensibilité et distance d'affichage bornées/persistées ; glyphes des panneaux vérifiés.
- Audio : fréquences, volumes et durées des effets générés vérifiés sans dépendre d'un périphérique audio.
- Crafting : limites d'inventaire, récolte/dépense, recettes de briques et de planches, ressources insuffisantes et capacité de sortie.
- Sauvegardes : aller-retour monde/inventaire/progression/joueur/phase jour-nuit/orientation caméra/munitions, migrations v1-v9 et refus des données hors limites ou dupliquées.
- Commandes de sauvegarde : vérifier le retour visible en cas de succès ou d'erreur F5/F9.
- Chunks : fenêtre de streaming bornée autour du joueur et des ennemis vivants, navigation entre chunks, éviction vers le cache disque, rechargement sans perte de blocs et fusion des chunks en cache dans F5/F9.
- Tests d'intégration : dossier `tests/`, pour le binaire et la sauvegarde (aller-retour dans le dossier temporaire du système, sans `tempfile`).
- Étapes graphiques (fenêtre, rendu) : vérification manuelle, décrite dans la PR.
- HUD : jauge de vie, adaptation de la barre rapide aux petites fenêtres et indicateurs de stock sont testés sans contexte graphique ; l'affichage reste à vérifier manuellement.
- Chaque étape de la ROADMAP ajoute ses tests avant clôture.

## Limites

- Les tests exigent un linker et les bibliothèques système de la cible. Sous Windows GNU, installez
  une toolchain MinGW-w64 complète (CRT et bibliothèques Windows) avant de lancer `cargo test`.
- Dans l'environnement de développement actuel, `cargo check --all-targets` et
  `cargo clippy --all-targets -- -D warnings` passent. `cargo test --all-targets` ne peut pas
  lier les exécutables de test : `x86_64-w64-mingw32-gcc` ne trouve pas le CRT (`crt2.o`,
  `crtbegin.o`) ni les bibliothèques Windows (`kernel32`, `opengl32`, `msvcrt`). Les tests sont
  donc présents mais restent à exécuter avec une toolchain MinGW-w64 complète.
- La fenêtre, le rendu et les interactions graphiques nécessitent une validation manuelle sur une
  machine avec affichage.
