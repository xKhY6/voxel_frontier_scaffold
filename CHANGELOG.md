## Unreleased

- Fenêtre winit 0.30 (`ApplicationHandler`) avec fermeture par la croix ou Échap.
- Boucle à pas fixe (60 Hz environ) avec rattrapage borné ; tests unitaires de l'horloge.
- Entrées : état clavier et souris (`input/`), alimenté par la fenêtre, avec tests unitaires.
- Caméra FPS (`camera/`) : rotation bornée, matrices vue et projection via glam, avec tests. Projection en profondeur -1..1 (OpenGL).
- Rendu OpenGL 3.3 core (`render/`) : fenêtre et contexte via glutin, appels via glow, shaders `assets/shaders/`, un maillage GPU par chunk, culling des faces arrière.
- Monde voxel (`voxel/`) : `BlockType`, `Chunk` en tableau plat de 16³ blocs, `World` indexe par coordonnées de chunk, lecture et écriture de blocs, avec tests (coordonnées négatives incluses).
- Maillage (`voxel/mesh.rs`) : faces exposées seulement, ombrage par direction, tests d'orientation et de faces cachées.
- Génération de terrain (`generation/`) : bruit de valeur déterministe par graine, zone de chunks, tests de raccord entre chunks.
- Déplacement au sol (`game/`) : WASD relatif au regard, gravité, saut (Espace) uniquement au sol ; intégration verticale testée (chute, saut).
- Collisions AABB (`physics/collision.rs`) : déplacement axe par axe avec glissement le long des murs, tests.
- Raycast DDA (`voxel/raycast.rs`) : premier bloc solide le long du regard, tests de face et de portée.
- Édition : clic gauche retire un bloc, clic droit pose de la pierre ; les chunks touchés sont remaillés.
- Sauvegarde JSON (`save/`, `serde_json`) : format version 1, un enregistrement par chunk ; F5 sauvegarde, F9 charge `voxel_frontier_save.json` ; aller-retour testé.
- Collisions : tolérance de contact (`CONTACT_TOLERANCE`) pour qu'un pied enfoncé d'un rien ne traverse plus le sol ; test de régression.
- Test de fumée : lance désormais le binaire avec `--check` au lieu de `assert!(true)`.
- Formatage du code avec rustfmt.
- Documentation : feuille de route MVP (11 étapes, critères, risques), couches, justification des dépendances, conventions de code, stratégie de tests.
- README : brief initial et état d'avancement.
- Feuille de route : thèmes V1.0 (génération avancée, éclairage, crafting, IA, shaders) planifiés sans code, avec prérequis.

0.1.0 Initial scaffold
