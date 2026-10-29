# Dépendances

Peu de dépendances, architecture simple.

Principe : minimum de crates. Une crate n'est ajoutée que si un développement interne serait déraisonnable, risqué ou trop complexe.

| Crate | Version (Cargo.lock) | Rôle | Justification | Section |
|---|---|---|---|---|
| winit | 0.30.13 | Fenêtre et événements | Gérer soi-même Win32, X11, Wayland et Cocoa serait déraisonnable | dependencies |
| glam | 0.30.10 | Vecteurs et matrices | Maths 3D optimisées ; une réécriture n'apporterait pas de valeur | dependencies |
| glow | 0.18.0 | Bindings OpenGL (appels GL) | Charger et appeler les fonctions OpenGL demande des bindings générés ; les écrire à la main serait risqué. Les appels restent `unsafe` et sont confinés à `render/` | dependencies |
| glutin | 0.32.3 | Contexte OpenGL et surface de rendu | La création de contexte dépend de la plateforme (WGL, EGL, GLX) ; la réimplémenter serait déraisonnable | dependencies |
| glutin-winit | 0.5.0 | Liaison entre glutin et winit | Évite de coder soi-même la configuration GL par fenêtre | dependencies |
| raw-window-handle | 0.6.2 | Handle de fenêtre partagé entre winit et glutin | Version imposée par winit et glutin ; aucun code propre | dependencies |
| serde | 1.0.229 | Sérialisation (feature `derive`) | La génération de code par derive est complexe à reproduire | dependencies |
| serde_json | 1.0.154 | Format JSON de sauvegarde | Format lisible et robuste ; à réévaluer, un format binaire maison peut suffire | dependencies |

Backend graphique : OpenGL 3.3 core via glow et glutin (voir [DECISIONS.md](DECISIONS.md)). `wgpu` a été écarté.

`serde` et `serde_json` sont utilisés par `save/` et par les dérives de `BlockType`. `tempfile` n'est pas retenu : les tests de sauvegarde utilisent le dossier temporaire du système.
