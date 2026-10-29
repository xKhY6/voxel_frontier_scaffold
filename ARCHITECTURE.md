# Architecture

Architecture modulaire : game, voxel, render, ai, crafting, lighting, generation.

## Point d'entrée

`src/main.rs` déclare `mod game;` et appelle `game::run()`. C'est le seul module actuellement compilé.

## Modules

| Module | Fichiers | État actuel | Compilé |
|---|---|---|---|
| game | src/game/mod.rs | Fenêtre winit et boucle à pas fixe ; `--check` affiche le message et quitte | oui |
| voxel | src/voxel/mod.rs, block.rs, chunk.rs, world.rs, mesh.rs, raycast.rs | `BlockType`, `Chunk` (tableau plat de 16³ blocs), `World` (table de chunks), coordonnées, maillage par chunk, raycast DDA | oui |
| render | src/render/mod.rs, src/render/context.rs ; assets/shaders/cube.vert, cube.frag | Fenêtre OpenGL 3.3 core (glutin, glow) ; `Renderer` dessine un maillage GPU par chunk avec la matrice vue-projection | oui |
| input | src/input/mod.rs | État clavier et souris, alimenté par la fenêtre ; lecture prévue par la caméra | oui |
| camera | src/camera/mod.rs | `Camera` FPS : rotation bornée, matrices vue et projection (glam) | oui |
| physics | src/physics/mod.rs, collision.rs | `Aabb` et `move_aabb` : collisions avec les blocs solides, déplacement axe par axe | oui |
| ai | src/ai/mod.rs | Commentaire seul | non |
| crafting | src/crafting/mod.rs | Commentaire seul | non |
| generation | src/generation/mod.rs | Terrain déterministe par graine (bruit de valeur), zone de chunks | oui |
| lighting | src/lighting/mod.rs | Commentaire seul | non |
| resources | src/resources/mod.rs | Commentaire seul | non |
| save | src/save/mod.rs | Sauvegarde JSON du monde (`save_world`, `load_world`, format version 1) ; F5 et F9 câblés dans `game/` | oui |
| config | src/config/ | Dossier vide | non |

Le dossier `assets/shaders/` contient les shaders du cube (GLSL 330 core) ; `assets/textures/` est vide.

## Couches (proposition)

Le brief impose de séparer moteur, logique de jeu et données. Répartition proposée, sans déplacement de fichiers pour l'instant :

- **Moteur** : `input`, `camera`, `render`, `physics` (primitives de collision), `lighting`.
- **Logique de jeu** : `game`, `generation`, `crafting`, `ai`.
- **Données** : `voxel`, `save`, `resources`.

Sens de dépendance autorisé : logique de jeu -> moteur -> données. Les données ne dépendent ni du moteur ni de la logique de jeu.

## Conséquences

- Les modules non déclarés ne sont ni compilés, ni testés, ni analysés par clippy.
- Les tests d'intégration de `tests/` ne peuvent pas importer le code du binaire, faute de `src/lib.rs`.

Ces deux points sont à trancher, voir [DECISIONS.md](DECISIONS.md).
