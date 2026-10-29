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

Le MVP (étapes 1 à 11) est terminé. Les systèmes V1.0 ci-dessous sont en cours d'intégration et
doivent satisfaire leurs critères avant que la version soit déclarée livrée.

| Thème | Critères d'acceptation (propositions) | Pré-requis | État |
|---|---|---|---|
| Génération procédurale avancée | Même graine et coordonnées donnent le même terrain ; biomes et relief raccordés entre chunks ; mémoire bornée pendant l'exploration | Étape 7, stockage des chunks | En cours : plaines, désert et toundra déterministes, arbres des plaines raccordés entre chunks ; chunks éloignés évincés dans un cache disque et fusionnés aux sauvegardes |
| Éclairage voxel | Propagation dans les chunks chargés ; ombres visibles ; recalcul après génération et édition ; tests des occlusions et frontières | Étape 9, stockage des chunks | En cours : lumière solaire CPU, cycle jour/nuit de 4 minutes, direction solaire et éclairage directionnel via shader ; ombres voxel recalculées après les changements du monde et seuls les maillages dont les faces sont affectées sont actualisés |
| Crafting et survie | Recettes définies en données ; récolte/pose/craft cohérents ; inventaire persistant ; interface lisible | Étape 10 (sauvegarde de l'inventaire) | En cours : inventaire borné avec neuf matériaux, récolte/pose, recettes de briques et de planches ; panneau de fabrication accessible par I avec ingrédients et disponibilité affichés ; recettes supplémentaires encore à faire |
| Combat et objectif | Ennemis lisibles, réactions, attaque, victoire/défaite et progression persistante ; tests des transitions et collisions | Étape 8 (collisions) | En cours : deux ennemis avec gravité/collisions et pathfinding ; mêlée, trois armes à cadences/portées/dégâts distincts, munitions finies, rechargement, tirs bloqués par les blocs, HUD de munitions et état d'arme sauvegardé ; victoire et progression persistante ; équilibrage à faire |
| Rendu et présentation | Shaders documentés ; HUD et retours visuels ; validation sur une fenêtre réelle | Backend graphique choisi (DECISIONS, point 2), étape 6 | En cours : shader OpenGL, éclairage directionnel suivant le soleil, ennemis, HUD et panneaux adaptatifs ; distance d'affichage réglable ; effets sonores synthétisés intégrés ; audio d'ambiance et validation graphique à faire |

Les critères sont des objectifs de livraison, à vérifier par tests et essai manuel ; le choix du backend graphique et le stockage des chunks conditionnent les thèmes concernés.

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
| 7 | Petit terrain | Génération déterministe (même seed, même monde) d'une zone de chunks | Fait (plaines, désert, toundra ; zone de streaming initiale générée avant l'affichage) |
| 8 | Déplacement et collisions | Le joueur ne traverse pas les blocs solides ; tests de collision AABB | Fait (déplacement au sol, gravité, saut, collisions AABB et retour après chute hors terrain) |
| 9 | Ajout et suppression de blocs | Raycast depuis la caméra ; modification du monde ; maillage mis à jour | Fait (raycast et maillage testés ; édition à la souris à confirmer à l'écran) |
| 10 | Sauvegarde et chargement | Aller-retour save puis load identique ; test dans le dossier temporaire du système | Fait (JSON v10 monde/inventaire/progression/joueur/cycle lumineux/orientation caméra/munitions, migrations v1-v9, F5/F9 ; aller-retour testé) |
| 11 | Tests et documentation | Tests des étapes 5 à 10 en place ; documentation à jour | Fait |

Pré-requis de l'étape 6 levé : backend OpenGL 3.3 choisi (voir [DECISIONS.md](DECISIONS.md)).

### État jouable actuel

La base est un bac à sable local : déplacement FPS avec collisions, saut, édition de blocs, neuf
matériaux sélectionnables, chargement de chunks autour du joueur, sauvegarde/chargement (F5/F9)
et retour au point de départ après une chute hors du terrain. La boucle de survie est jouable :
miner ajoute le bloc à l'inventaire, poser en consomme un, C fabrique quatre briques avec deux
pierres et deux terres, et V transforme une bûche en quatre planches.
La touche P suspend la simulation et le cycle lumineux ; un indicateur de pause apparaît au centre.
Un menu de démarrage propose une nouvelle partie, la reprise d'une sauvegarde présente et la sortie.
Vaincre les deux ennemis termine maintenant la partie, affiche un panneau de victoire et R recommence
avec monde, inventaire et cache réinitialisés.
L'inventaire, les ennemis vaincus et l'état du joueur (dont la phase jour/nuit, l'orientation de la
caméra et les munitions) sont inclus dans la sauvegarde v10 ; les sauvegardes v1-v9 restent chargeables. La
génération produit trois biomes déterministes avec sable, neige et arbres récoltables. L'éclairage solaire
est propagé dans les cellules d'air et ombre les faces ; un cycle jour/nuit assombrit les blocs et le ciel ; deux ennemis visibles poursuivent le joueur,
attaquent au contact et peuvent être vaincus avec F ou par tirs. Trois armes ont des chargeurs et réserves limités ; Q/1-3 les sélectionnent, R recharge et les blocs arrêtent les tirs. La barre de titre résume l'inventaire et les commandes ; le panneau de fabrication (I) affiche les recettes et les ingrédients et suspend le jeu pendant son ouverture ; le viseur, la vie, les ennemis et la barre
rapide visuelle des matériaux sont aussi indiqués à l'écran. En pause, un panneau d'options permet
d'ajuster la sensibilité de la souris sur cinq niveaux et la distance d'affichage sur trois niveaux ; ces réglages sont conservés dans le fichier d'options local.

Ce n'est pas encore la V1.0 : profilage des performances du streaming (la fenêtre de chunks retenue est testée),
pathfinding complet
(notamment la gestion des chunks non chargés), équilibrage complémentaire du combat (cadence joueur,
protection après dégâts et écran de fin ajoutés),
recettes supplémentaires,
ombres projetées suivant l'orientation solaire, audio d'ambiance, options graphiques complémentaires et validation graphique restent à implémenter. L'objectif jouable actuel est
d'éliminer les deux ennemis de départ. Le multijoueur reste hors périmètre V1.0.

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

- Vérification graphique manuelle de l'édition, de la capture souris, du combat, du streaming et des sauvegardes à confirmer sur une machine avec affichage.
- Fonctionnalités V1.0 restantes listées dans le tableau ci-dessus ; le statut doit rester explicite jusqu'à leur validation.
