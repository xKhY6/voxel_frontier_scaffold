# Tests

## Commandes

- `cargo test` : lance tous les tests.
- `cargo clippy --all-targets` : doit sortir sans warning.
- `cargo fmt --check` : vérifie le format (configuration dans `rustfmt.toml`).

## Tests existants

- `tests/smoke_test.rs` (`smoke`) : lance le binaire avec `--check` et vérifie qu'il sort avec succès en affichant « Voxel Frontier bootstrap », sans ouvrir de fenêtre.

## Stratégie

- Tests unitaires : logique pure (voxel, collisions, caméra, génération seedée), à côté du code.
- Tests d'intégration : dossier `tests/`, pour le binaire et la sauvegarde (aller-retour dans le dossier temporaire du système, sans `tempfile`).
- Étapes graphiques (fenêtre, rendu) : vérification manuelle, décrite dans la PR.
- Chaque étape de la ROADMAP ajoute ses tests avant clôture.

## Limites

- Les tests unitaires placés dans `src/voxel`, `src/render`, etc. ne sont pas exécutés tant que ces modules ne sont pas déclarés dans `main.rs` (voir [ARCHITECTURE.md](ARCHITECTURE.md)).
