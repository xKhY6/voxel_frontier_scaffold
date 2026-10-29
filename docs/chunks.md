# Stratégie de chunks

## État actuel

- `CHUNK_SIZE = 16` (`src/voxel/chunk.rs`) : taille d'un chunk en blocs, par axe.
- `BlockType` (`src/voxel/block.rs`) : Air, Grass, Dirt, Stone.
- `Chunk`, `World`, la génération, le maillage et l'édition sont implémentés et testés (étapes 7 à 9).

## Contraintes

- `CHUNK_SIZE` est la source unique de la taille : ne pas la recopier ailleurs.
- Un chunk contient `CHUNK_SIZE³` blocs (4096 avec la valeur actuelle).

## Choix retenus

- Stockage : tableau plat `Vec<BlockType>` de `CHUNK_VOLUME` cases, index `x + S * (y + S * z)` avec `S = CHUNK_SIZE`. Structure hiérarchique écartée pour le MVP.
- Coordonnées : `split_block_pos` sépare une position monde (`IVec3`) en coordonnée de chunk et position locale, par division euclidienne (les négatifs tombent dans le bon chunk).
- `World` : table `HashMap<IVec3, Chunk>` ; un chunk absent est traité comme de l'air.
- `World::set_block` refuse d'écrire dans un chunk non chargé ; `chunks_touching` liste les chunks à remailler après une écriture (le chunk et ses voisins si le bloc est sur un bord).

## Génération

- Bruit de valeur 2D sur (x, z), deux octaves, graine `u64`, sans crate externe.
- Hauteur de surface entre 8 et 23 blocs ; sous l'herbe, 3 blocs de terre puis de la pierre ; au-dessus, de l'air.
- Zone : chunks en x et z dans `[-r, r[`, couches y = 0 et 1.

## À décider

- Chargement et déchargement des chunks autour du joueur (hors MVP : zone fixe).
