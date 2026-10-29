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
- Phase 2 (implémentation) : étapes 1 à 10 sur 11 faites (édition à la souris à confirmer à l'écran ; F5 sauvegarde, F9 charge) ; voir [ROADMAP.md](ROADMAP.md).

## Démarrage

Prérequis : toolchain Rust stable (rustup).

```
cargo run
cargo test
```

## Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md) : modules et état d'avancement
- [ROADMAP.md](ROADMAP.md) : feuille de route
- [TESTING.md](TESTING.md) : tests et qualité
- [DEPENDENCIES.md](DEPENDENCIES.md) : dépendances
- [DECISIONS.md](DECISIONS.md) : décisions et points ouverts
- [CONTRIBUTING.md](CONTRIBUTING.md) : contribution
- [CHANGELOG.md](CHANGELOG.md) : historique
- [docs/](docs/) : notes moteur, chunks et rendu
