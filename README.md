# Voxel Frontier
Scaffold documenté pour jeu voxel Rust.

## Brief initial

Je souhaite créer en Rust un jeu 3D inspiré de Minecraft et de Fortnite, avec un monde composé de blocs, de la construction et, à terme, des mécaniques d’action.

L’objectif est d’obtenir une application performante, modulaire et maintenable, en utilisant le minimum de dépendances externes. Chaque dépendance proposée devra être justifiée et ne devra être ajoutée que si son développement en interne serait déraisonnable, risqué ou trop complexe.

Procède en deux phases.

PHASE 1 — Conception du projet

1. Reformule les objectifs et délimite un MVP réaliste.
2. Propose les choix techniques nécessaires :
   - moteur de rendu ;
   - gestion des fenêtres et des entrées ;
   - génération et affichage du monde en blocs ;
   - architecture du moteur et du jeu ;
   - sauvegarde locale ;
   - physique et collisions ;
   - multijoueur éventuel.
3. Présente les dépendances Rust envisagées et justifie chacune d’elles.
4. Crée une arborescence de projet claire et évolutive.
5. Prépare la documentation nécessaire :
   - README.md ;
   - ARCHITECTURE.md ;
   - ROADMAP.md ;
   - CONTRIBUTING.md ;
   - TESTING.md ;
   - conventions de code ;
   - journal des décisions techniques.
6. Définis les étapes de développement, les critères d’acceptation et les risques techniques.
7. Vérifie la cohérence globale de la proposition.

Ne génère pas encore l’intégralité du code pendant cette phase.

PHASE 2 — Implémentation

Une fois la conception terminée, commence directement l’implémentation du MVP dans cet ordre :

1. initialisation du projet Rust ;
2. création d’une fenêtre ;
3. boucle principale du jeu ;
4. gestion des entrées ;
5. caméra 3D ;
6. affichage d’un premier bloc ;
7. génération d’un petit terrain ;
8. déplacement et collisions ;
9. ajout et suppression de blocs ;
10. sauvegarde et chargement du monde ;
11. tests et documentation.

Contraintes :

- utiliser la version stable de Rust ;
- limiter au maximum le nombre de crates ;
- éviter les dépendances redondantes ;
- privilégier une architecture simple avant toute abstraction avancée ;
- séparer clairement le moteur, la logique du jeu et les données ;
- produire du code compilable à chaque étape ;
- ajouter les tests pertinents au fur et à mesure ;
- documenter les commandes d’installation, de compilation et d’exécution ;
- signaler clairement toute hypothèse ou fonctionnalité volontairement reportée.

À la fin de chaque étape, indique :
- les fichiers créés ou modifiés ;
- les choix réalisés ;
- les commandes à exécuter ;
- les tests effectués ;
- la prochaine étape.
## État d'avancement

- Phase 1 (conception) : documentée ; points ouverts dans [DECISIONS.md](DECISIONS.md).
- Phase 2 (implémentation) : MVP (étapes 1 à 11) terminé ; travail V1.0 en cours, avec récolte, inventaire et recettes de briques/planches jouables. Voir [ROADMAP.md](ROADMAP.md).

## Démarrage

Prérequis : toolchain Rust stable (rustup).

```
cargo run
cargo test
```

## Commandes en jeu

- Au démarrage : **N** nouvelle partie, **C** reprendre la sauvegarde si disponible, **Échap** quitter.
- **WASD** : se déplacer ; **Espace** : sauter.
- **Premier clic** : capturer la souris pour regarder ; **Échap** : libérer la souris ; Échap une seconde fois ferme le jeu.
- **Clic gauche** : miner et récupérer le bloc visé ; **clic droit** : poser un bloc depuis l’inventaire.
- **1 à 9** : choisir herbe, terre, pierre, brique, sable, neige, bois, feuilles ou planches.
- **C** : fabriquer 4 briques avec 2 pierres et 2 terres ; **V** : fabriquer 4 planches avec 1 bûche.
- **I** : ouvrir le panneau de fabrication ; les ingrédients disponibles y sont indiqués et les recettes réalisables mises en évidence. La partie est suspendue pendant son ouverture ; **I** ou **Échap** ferme le panneau.
- **Q** : passer en mode armes (pistolet, fusil ou fusil à pompe) ; appuyez encore sur **Q** pour parcourir les armes, ou sur **1 à 3** pour en choisir une. **E** revient au mode construction.
- **Clic gauche** en mode armes : tirer dans la direction du viseur ; les blocs arrêtent les tirs. Les armes ont des cadences, portées, dégâts et chargeurs différents ; les munitions restantes sont affichées au-dessus de la barre rapide. **R** recharge avec les réserves finies.
- **F** : frapper l’ennemi situé devant vous (une frappe toutes les 0,3 s) ; les ennemis proches poursuivent et attaquent. Après un dégât, le joueur bénéficie de 0,65 s de protection, y compris contre les coups simultanés.
- **F5 / F9** : sauvegarder / charger `voxel_frontier_save.json` dans le dossier courant ; le résultat ou l’erreur est reporté dans la barre de titre.
- **P** : mettre en pause ou reprendre ; **Échap** en pause quitte le jeu.
- En pause, **O** ouvre les options ; gauche/droite règlent la sensibilité de la souris sur cinq niveaux et haut/bas règlent la distance d'affichage de 1 à 3 chunks. Les deux options sont conservées dans `voxel_frontier_settings.json`. **O** ou **Échap** revient à la pause, **P** reprend.
- Après avoir vaincu les deux ennemis, la simulation s'arrête sur l'écran de victoire ; **R** démarre une nouvelle partie.
- Les chunks éloignés sont évincés en cache temporaire puis réintégrés à F5/F9 ; les blocs modifiés sont conservés. Les chunks proches des ennemis vivants restent chargés pour leur déplacement aux frontières de chunks.
- Une chute sous le monde ramène le joueur au point de départ.

Le HUD affiche le viseur, la santé, les ennemis restants, les munitions de l'arme active, une barre rapide visuelle et un écran de victoire : le bloc
sélectionné est encadré et cinq repères par emplacement indiquent les premières unités disponibles.
La barre de titre rappelle aussi les commandes, les quantités exactes de matériaux et le bloc sélectionné.
L’inventaire, la progression des ennemis, la position, la santé et les munitions des armes sont conservés dans la sauvegarde
v10 ; les sauvegardes v1 à v9 restent chargeables. La phase jour/nuit et l’orientation de la caméra
sont sauvegardées.
Le terrain comprend plaines, déserts et toundras, arbres récoltables, cycle jour/nuit et ennemis ; l'orientation solaire anime aussi l'éclairage directionnel des faces. La V1.0 est
en cours : voir [ROADMAP.md](ROADMAP.md) pour les systèmes encore manquants.

## Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md) : modules et état d'avancement
- [ROADMAP.md](ROADMAP.md) : feuille de route
- [TESTING.md](TESTING.md) : tests et qualité
- [DEPENDENCIES.md](DEPENDENCIES.md) : dépendances
- [DECISIONS.md](DECISIONS.md) : décisions et points ouverts
- [CONTRIBUTING.md](CONTRIBUTING.md) : contribution
- [CHANGELOG.md](CHANGELOG.md) : historique
- [docs/](docs/) : notes moteur, chunks et rendu
