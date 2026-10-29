# Feuille de route

## Objectifs

Jeu 3D inspiré de Minecraft et Fortnite : monde composé de blocs, construction, puis mécaniques d'action à terme. L'application doit rester performante, modulaire et maintenable, avec un minimum de dépendances externes.

Trajectoire : V0.1 MVP -> V1.0 Sandbox complet.

## Périmètre du MVP (V0.1)

Inclus :

- fenêtre et boucle principale ;
- entrées clavier et souris ;
- caméra 3D ;
- affichage d'un bloc, puis d'un petit terrain généré ;
- déplacement avec collisions ;
- ajout et suppression de blocs ;
- sauvegarde et chargement locaux.

Reporté (hors MVP) :

- multijoueur ;
- mécaniques d'action (combat, déplacement avancé) ;
- IA, craft et inventaire, éclairage voxel, physique au-delà des collisions AABB.

Ces points restent dans l'arborescence pour la V1.0, mais ne sont pas planifiés dans le MVP.

## Périmètre V1.0 (après le MVP)

Ces thèmes ne démarrent qu'après l'étape 11 du MVP. Ils dépendent des étapes 6 à 10 (rendu, monde, collisions, sauvegarde).

| Thème | Critères d'acceptation (propositions) | Pré-requis |
|---|---|---|
| Génération procédurale avancée | Bruit déterministe (même seed, même monde) ; biomes ou relief multi-octaves ; tests de reproductibilité | Étape 7, stockage des chunks (DECISIONS, point 3) |
| Éclairage voxel | Propagation de la lumière par flood-fill sur les chunks ; mise à jour lors d'un changement de bloc ; tests de propagation | Étape 9, stockage des chunks |
| Crafting | Recettes définies en données ; inventaire ; tests de validation des recettes | Étape 10 (sauvegarde de l'inventaire) |
| IA | Machine d'état simple pour les entités ; tests de transitions | Étape 8 (collisions) |
| Shaders avancés | Shaders chargés depuis `assets/shaders/` ; éclairage et ombres dans le shader ; contrôle manuel | Backend graphique choisi (DECISIONS, point 2), étape 6 |

Les critères sont à valider avant implémentation. Le choix du backend graphique et le stockage des chunks conditionnent les trois premiers thèmes.

## Étapes de développement (Phase 2)

Les critères d'acceptation sont des propositions à valider.

| # | Étape | Critères d'acceptation | État |
|---|---|---|---|
| 1 | Initialisation du projet Rust | `cargo build` et `cargo test` passent ; arborescence en place | Fait |
| 2 | Création d'une fenêtre | La fenêtre s'ouvre et se ferme proprement ; `cargo run` ne panique pas | Fait (ouverture vérifiée ; fermeture à confirmer à l'écran) |
| 3 | Boucle principale | Boucle update/render à pas de temps ; sortie par fermeture de fenêtre ou Échap | Fait (horloge à pas fixe testée ; rendu vide) |
| 4 | Gestion des entrées | Événements clavier et souris traduits en état dans `input/` ; test unitaire de l'état | Fait (état testé, branché à la fenêtre) |
| 5 | Caméra 3D | Caméra FPS produisant des matrices vue/projection (glam) ; tests des matrices | Fait (rotation et matrices testées ; déplacement à l'étape 8) |
| 6 | Premier bloc | Un cube est affiché à l'écran | Fait (rendu OpenGL 3.3 ; cube vérifié à l'écran) |
| 7 | Petit terrain | Génération déterministe (même seed, même monde) d'une zone de chunks | Fait (génération testée ; terrain affiché à l'écran) |
| 8 | Déplacement et collisions | Le joueur ne traverse pas les blocs solides ; tests de collision AABB | Fait (déplacement au sol, gravité et saut, collisions AABB testées ; chute hors zone chargée non traitée, voir DECISIONS) |
| 9 | Ajout et suppression de blocs | Raycast depuis la caméra ; modification du monde ; maillage mis à jour | Fait (raycast et maillage testés ; édition à la souris à confirmer à l'écran) |
| 10 | Sauvegarde et chargement | Aller-retour save puis load identique ; test dans le dossier temporaire du système | Fait (JSON version 1 via serde_json, F5 et F9 ; aller-retour testé) |
| 11 | Tests et documentation | Tests des étapes 5 à 10 en place ; documentation à jour | À faire |

Pré-requis de l'étape 6 levé : backend OpenGL 3.3 choisi (voir [DECISIONS.md](DECISIONS.md)).

## Risques techniques

| Risque | Impact | Parade |
|---|---|---|
| Backend graphique | Impact sur les étapes 6 à 11 | Choisi : OpenGL 3.3 via glow et glutin (DECISIONS, point 2) ; à valider sur les plateformes cibles |
| Modules non compilés | Dérive entre code et documentation | Câbler les modules (DECISIONS, point 1) |
| Coût du rendu GPU maison | Délais, bugs de pipeline | Limiter les dépendances au strict nécessaire, justifier chaque ajout |
| Performance du maillage | Chute de FPS dès plusieurs chunks | Ne reconstruire que les chunks modifiés |
| Cas limites des collisions et du raycast | Traversée de blocs, sélection erronée | Tests dédiés sur les bords de blocs |
| Format de sauvegarde | Incompatibilité lors des évolutions | Numéro de version du format dès le départ |
| Plateforme | Seul Windows a été vérifié | Tester sur d'autres OS avant la V1.0 |

## Dette technique

- Modules non déclarés dans `main.rs` (voir [ARCHITECTURE.md](ARCHITECTURE.md)).
- Thèmes V1.0 sans code : ils attendent les décisions 1 à 3 de [DECISIONS.md](DECISIONS.md).
