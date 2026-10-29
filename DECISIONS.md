# Décisions

## Décisions prises

- Minimum de dépendances externes. Toute crate ajoutée est justifiée dans [DEPENDENCIES.md](DEPENDENCIES.md).
- Toolchain Rust stable uniquement.
- Architecture simple avant toute abstraction avancée.
- Séparation entre moteur, logique de jeu et données (voir [ARCHITECTURE.md](ARCHITECTURE.md)).
- Multijoueur hors MVP.
- Contributions par fork puis PR (voir [CONTRIBUTING.md](CONTRIBUTING.md)).
- Backend graphique : OpenGL 3.3 core, appels via glow et contexte via glutin (choix de l'utilisateur ; `wgpu` écarté). Profondeur en -1..1 (`Mat4::perspective_rh_gl`).
- Chunks : tableau plat de `CHUNK_SIZE³` blocs, positions monde séparées des positions locales (retenu pour le MVP, voir [docs/chunks.md](docs/chunks.md)).
- Déplacement au sol : WASD relatif au regard, gravité de 28 blocs/s², saut de 8,5 blocs/s uniquement au sol (Espace). Constantes dans `game/mod.rs` (voir point 8 ci-dessous).
- Collisions : boîte AABB du joueur, déplacement axe par axe (X, Y, Z) avec glissement le long des murs.
- Édition : clic gauche récolte le bloc visé dans l'inventaire, clic droit dépense le bloc sélectionné pour le poser sur la face visée, portée de 6 blocs (raycast DDA). Pas de pose dans la boîte du joueur.
- Maillage : reconstruction complète du chunk modifié et de ses voisins sur un bord (`chunks_touching`), sans maillage incrémental au MVP.

## À trancher

1. **Câblage des modules** : tranché. Modules déclarés dans `main.rs`, pas de `src/lib.rs` ; les tests de `tests/` ne testent que le binaire via `--check`.
2. **Backend graphique** : tranché (voir ci-dessus). Reste à vérifier que le contexte OpenGL 3.3 core est disponible sur chaque plateforme cible (point 6).
3. **Stockage des chunks** : tranché (tableau plat, voir ci-dessus et [docs/chunks.md](docs/chunks.md)).
4. **Format de sauvegarde** : JSON via `serde_json`, chunks, inventaire, ennemis vaincus et état du joueur ; format v9. Les versions 1 à 8 restent chargeables avec migrations ; F5 écrit v9 et F9 charge `voxel_frontier_save.json`.
5. **Découpage en couches** : valider le regroupement proposé dans [ARCHITECTURE.md](ARCHITECTURE.md) (moteur, logique de jeu, données), ou garder l'arborescence plate actuelle.
6. **Plateforme cible** : seul Windows a été vérifié. Définir les OS supportés pour la V0.1.
7. **Thèmes V1.0** (génération avancée, éclairage, crafting, IA, shaders) : prévus dans la feuille de route ; génération par biomes et crafting ont commencé. La progression vers V1.0 reste conditionnée par les critères de [ROADMAP.md](ROADMAP.md). Le backend graphique est choisi (point 2) : OpenGL, `wgpu` écarté.
8. **Gravité et saut** : tranchés, ajoutés au MVP à la demande de l'utilisateur. Hors de la zone chargée, le joueur tombe ; sous y = -32 il réapparaît au point de départ. Le chargement de chunks reste à traiter.
