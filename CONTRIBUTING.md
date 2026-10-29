Fork puis PR.

## Avant d'ouvrir une PR

1. `cargo fmt --check` ne produit aucun diff.
2. `cargo clippy --all-targets` ne produit aucun warning.
3. `cargo test` passe.
4. CHANGELOG.md (section Unreleased) et la documentation concernée sont mis à jour.

## Conventions

- Documentation et commentaires en français.
- Commenter les parties modifiées : intention, contrats, contraintes et cas limites, sans paraphraser le code.

## Conventions de code

- Formatage : `cargo fmt` (largeur maximale 100, voir `rustfmt.toml`).
- Lint : `cargo clippy --all-targets` sans warning ; 7 arguments maximum par fonction (voir `clippy.toml`).
- Nommage Rust standard : `snake_case` pour fonctions et modules, `CamelCase` pour types.
- Hors tests, préférer `Result` à `unwrap()` et `expect()`.
- Moteur, logique de jeu et données restent séparés (voir [ARCHITECTURE.md](ARCHITECTURE.md)).
- Toute nouvelle crate est justifiée dans [DEPENDENCIES.md](DEPENDENCIES.md).
- Chaque étape de la [ROADMAP.md](ROADMAP.md) reste compilable : `cargo build` passe à chaque commit.
