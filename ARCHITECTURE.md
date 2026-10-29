# Architecture

Architecture modulaire : game, voxel, render, audio, ai, crafting, lighting, generation.

## Point d'entrée

`src/main.rs` déclare les modules utilisés par le jeu et appelle `game::run()`. Les modules `audio`, `ai`, `crafting` et `lighting` sont actifs ; `resources` reste hors compilation.

## Modules

| Module | Fichiers | État actuel | Compilé |
|---|---|---|---|
| game | src/game/mod.rs | Fenêtre winit et boucle à pas fixe ; survie, streaming joueur/ennemis, mêlée et tirs, menus, pause, options souris et distance d'affichage persistées, sauvegarde/chargement complet ; `--check` affiche le message et quitte | vérification/clippy passés, tests non liés |
| audio | src/audio/mod.rs | Effets courts générés sans fichiers son et déclenchés par les actions ; absence de périphérique signalée sans empêcher le jeu ; audio d'ambiance à faire | vérification/clippy passés, tests non liés |
| voxel | src/voxel/mod.rs, block.rs, chunk.rs, world.rs, mesh.rs, raycast.rs | `BlockType`, `Chunk` (tableau plat de 16³ blocs), `World` (table de chunks), coordonnées, maillage par chunk avec normales, raycast DDA | oui |
| render | src/render/mod.rs, src/render/context.rs ; assets/shaders/cube.vert, cube.frag | Fenêtre OpenGL 3.3 core (glutin, glow) ; maillages par chunk, cubes d'entités, éclairage directionnel et panneaux/HUD, dont les options de distance d'affichage | vérification/clippy passés, tests non liés |
| input | src/input/mod.rs | État clavier et souris, alimenté par la fenêtre et consommé par la boucle de jeu | oui |
| camera | src/camera/mod.rs | `Camera` FPS : rotation bornée, matrices vue et projection (glam) | oui |
| physics | src/physics/mod.rs, collision.rs | `Aabb` et `move_aabb` : collisions avec les blocs solides, déplacement axe par axe | oui |
| ai | src/ai/mod.rs | Ennemi voxel à états, gravité/collisions, pathfinding borné avec montée de marches, arrêt sûr sans chemin, attaques/ciblage limités en hauteur | oui |
| weapons | src/weapons.rs | Trois armes fictives avec cadence, portée, dégâts, chargeurs, réserves et état sérialisable | oui |
| crafting | src/crafting/mod.rs, assets/recipes.json | Inventaire borné à neuf matériaux et recettes chargées depuis les données intégrées ; tests de récolte, dépense et fabrication | oui |
| generation | src/generation/mod.rs | Terrain déterministe par graine avec plaines, déserts, toundras, arbres et relief multi-octaves | oui |
| lighting | src/lighting/mod.rs | Propagation solaire flood-fill, cycle jour/nuit, direction solaire et détection des maillages affectés par une variation de lumière | vérification/clippy passés, tests non liés |
| resources | src/resources/mod.rs | Commentaire seul | non |
| save | src/save/mod.rs | Sauvegarde JSON du monde, de l'inventaire, de la progression, de l'état joueur, de la phase lumineuse, de l'orientation caméra et des munitions (format v10, migrations v1-v9) ; cache disque des chunks évincés fusionné à F5/F9 | oui |

Les options souris sont stockées dans `voxel_frontier_settings.json` dans le dossier courant. Le dossier `assets/shaders/` contient les shaders du cube (GLSL 330 core) ; `assets/textures/` est vide.

## Couches (proposition)

Le brief impose de séparer moteur, logique de jeu et données. Répartition proposée, sans déplacement de fichiers pour l'instant :

- **Moteur** : `input`, `camera`, `render`, `audio`, `physics` (primitives de collision), `lighting`.
- **Logique de jeu** : `game`, `generation`, `crafting`, `ai`.
- **Données** : `voxel`, `save`, `resources`.

Sens de dépendance autorisé : logique de jeu -> moteur -> données. Les données ne dépendent ni du moteur ni de la logique de jeu.

## Conséquences

- Les modules non déclarés ne sont ni compilés, ni testés, ni analysés par clippy.
- Les tests d'intégration de `tests/` ne peuvent pas importer le code du binaire, faute de `src/lib.rs`.

Ces deux points sont à trancher, voir [DECISIONS.md](DECISIONS.md).
