//! Monde : table de chunks indexée par leurs coordonnées de chunk.

use std::collections::HashMap;

use glam::IVec3;

use super::block::BlockType;
use super::chunk::{Chunk, CHUNK_SIZE};

/// Coordonnées d'un chunk dans le monde : un pas vaut `CHUNK_SIZE` blocs.
pub type ChunkCoord = IVec3;

/// Sépare une position de bloc monde en coordonnées de chunk et position locale.
/// Utilise la division euclidienne : les positions négatives tombent dans le chunk voisin de gauche, et la position locale reste dans `0..CHUNK_SIZE`.
pub fn split_block_pos(block: IVec3) -> (ChunkCoord, IVec3) {
    let size = IVec3::splat(CHUNK_SIZE as i32);
    (block.div_euclid(size), block.rem_euclid(size))
}

/// Chunks chargés. Un chunk absent est traité comme de l'air.
#[derive(Debug, Default, PartialEq)]
pub struct World {
    chunks: HashMap<ChunkCoord, Chunk>,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insère le chunk à `coord`, en remplaçant l'éventuel chunk existant.
    pub fn insert_chunk(&mut self, coord: ChunkCoord, chunk: Chunk) {
        self.chunks.insert(coord, chunk);
    }

    pub fn chunk(&self, coord: ChunkCoord) -> Option<&Chunk> {
        self.chunks.get(&coord)
    }

    /// Bloc à une position monde. `Air` si le chunk n'est pas chargé.
    pub fn block_at(&self, block: IVec3) -> BlockType {
        let (coord, local) = split_block_pos(block);
        self.chunk(coord)
            .and_then(|chunk| chunk.get(local))
            .unwrap_or(BlockType::Air)
    }

    /// Remplace le bloc à une position monde. `false` si le chunk n'est pas chargé.
    pub fn set_block(&mut self, block: IVec3, value: BlockType) -> bool {
        let (coord, local) = split_block_pos(block);
        match self.chunks.get_mut(&coord) {
            Some(chunk) => chunk.set(local, value),
            None => false,
        }
    }

    /// Coordonnées de tous les chunks chargés.
    pub fn chunk_coords(&self) -> Vec<ChunkCoord> {
        self.chunks.keys().copied().collect()
    }
}

/// Chunks à reconstruire après un changement du bloc `block` : son chunk, et les voisins s'il est sur un bord.
pub fn chunks_touching(block: IVec3) -> Vec<ChunkCoord> {
    let (coord, local) = split_block_pos(block);
    let mut coords = Vec::new();
    for dx in touching_offsets(local.x) {
        for dy in touching_offsets(local.y) {
            for dz in touching_offsets(local.z) {
                coords.push(coord + IVec3::new(*dx, *dy, *dz));
            }
        }
    }
    coords
}

/// Décalages de chunk à considérer sur un axe, selon la position locale du bloc.
fn touching_offsets(local: i32) -> &'static [i32] {
    let last = CHUNK_SIZE as i32 - 1;
    if local == 0 {
        &[0, -1]
    } else if local == last {
        &[0, 1]
    } else {
        &[0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_decoupe_gere_les_coordonnees_negatives() {
        let size = CHUNK_SIZE as i32;

        assert_eq!(
            split_block_pos(IVec3::new(-1, 0, size)),
            (IVec3::new(-1, 0, 1), IVec3::new(size - 1, 0, 0))
        );
    }

    #[test]
    fn la_decoupe_aux_bords_de_chunk() {
        let size = CHUNK_SIZE as i32;

        assert_eq!(split_block_pos(IVec3::ZERO), (IVec3::ZERO, IVec3::ZERO));
        assert_eq!(
            split_block_pos(IVec3::splat(size - 1)),
            (IVec3::ZERO, IVec3::splat(size - 1))
        );
        assert_eq!(split_block_pos(IVec3::splat(size)).0, IVec3::ONE);
    }

    #[test]
    fn un_chunk_absent_est_de_l_air() {
        let world = World::new();

        assert_eq!(world.block_at(IVec3::new(5, 5, 5)), BlockType::Air);
    }

    #[test]
    fn block_at_lit_le_chunk_insere_y_compris_en_negatif() {
        let size = CHUNK_SIZE as i32;
        let mut world = World::new();
        world.insert_chunk(IVec3::new(-1, 0, 0), Chunk::filled(BlockType::Stone));

        assert_eq!(world.block_at(IVec3::new(-1, 0, 0)), BlockType::Stone);
        assert_eq!(world.block_at(IVec3::new(-size, 0, 0)), BlockType::Stone);
        assert_eq!(world.block_at(IVec3::new(-size - 1, 0, 0)), BlockType::Air);
        assert_eq!(world.block_at(IVec3::new(0, 0, 0)), BlockType::Air);
    }

    #[test]
    fn set_block_exige_un_chunk_charge() {
        let mut world = World::new();
        assert!(!world.set_block(IVec3::ZERO, BlockType::Stone));

        world.insert_chunk(IVec3::new(-1, 0, 0), Chunk::filled(BlockType::Air));
        assert!(world.set_block(IVec3::new(-1, 3, 3), BlockType::Stone));

        assert_eq!(world.block_at(IVec3::new(-1, 3, 3)), BlockType::Stone);
        assert_eq!(world.block_at(IVec3::new(-2, 3, 3)), BlockType::Air);
    }

    #[test]
    fn un_bloc_de_bord_touche_le_chunk_voisin() {
        let touched = chunks_touching(IVec3::new(0, 5, 5));

        assert_eq!(touched.len(), 2);
        assert!(touched.contains(&IVec3::new(-1, 0, 0)));
    }

    #[test]
    fn un_bloc_d_angle_touche_sept_voisins() {
        let size = CHUNK_SIZE as i32;

        assert_eq!(chunks_touching(IVec3::splat(size - 1)).len(), 8);
        assert_eq!(chunks_touching(IVec3::splat(5)).len(), 1);
    }
}
