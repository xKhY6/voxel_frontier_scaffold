//! Chunk : cube de `CHUNK_SIZE` blocs de côté, stocké dans un tableau plat.

use glam::IVec3;

use super::block::BlockType;

/// Taille d'un chunk en blocs, par axe. Source unique de la taille.
pub const CHUNK_SIZE: usize = 16;

/// Nombre de blocs d'un chunk.
pub const CHUNK_VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;

/// Taille en `i32`, pour les calculs sur coordonnées signées.
const SIZE: i32 = CHUNK_SIZE as i32;

/// Blocs d'un chunk. L'index est `x + SIZE * (y + SIZE * z)` : x varie le plus vite.
#[derive(Clone, Debug, PartialEq)]
pub struct Chunk {
    blocks: Vec<BlockType>,
}

impl Chunk {
    /// Chunk rempli d'un seul type de bloc.
    pub fn filled(block: BlockType) -> Self {
        Self {
            blocks: vec![block; CHUNK_VOLUME],
        }
    }

    /// Bloc à la position locale `local`, chaque axe dans `0..CHUNK_SIZE`. `None` hors du chunk.
    pub fn get(&self, local: IVec3) -> Option<BlockType> {
        index(local).map(|i| self.blocks[i])
    }

    /// Remplace le bloc à la position locale `local`. Retourne `false` si la position est hors du chunk.
    pub fn set(&mut self, local: IVec3, block: BlockType) -> bool {
        match index(local) {
            Some(i) => {
                self.blocks[i] = block;
                true
            }
            None => false,
        }
    }

    /// Blocs du chunk dans l'ordre de l'index plat.
    pub fn blocks(&self) -> &[BlockType] {
        &self.blocks
    }

    /// Chunk construit depuis des blocs déjà ordonnés. `None` si la longueur n'est pas `CHUNK_VOLUME`.
    pub fn from_blocks(blocks: Vec<BlockType>) -> Option<Self> {
        (blocks.len() == CHUNK_VOLUME).then_some(Self { blocks })
    }
}

/// Index plat d'une position locale, ou `None` si un axe sort de `0..CHUNK_SIZE`.
fn index(local: IVec3) -> Option<usize> {
    let inside = |v: i32| (0..SIZE).contains(&v);
    if !(inside(local.x) && inside(local.y) && inside(local.z)) {
        return None;
    }
    Some((local.x + SIZE * (local.y + SIZE * local.z)) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_chunk_rempli_renvoie_le_bloc_partout() {
        let chunk = Chunk::filled(BlockType::Stone);

        assert_eq!(chunk.get(IVec3::ZERO), Some(BlockType::Stone));
        assert_eq!(chunk.get(IVec3::splat(SIZE - 1)), Some(BlockType::Stone));
    }

    #[test]
    fn les_positions_hors_du_chunk_sont_refusees() {
        let mut chunk = Chunk::filled(BlockType::Air);

        assert_eq!(chunk.get(IVec3::new(-1, 0, 0)), None);
        assert_eq!(chunk.get(IVec3::new(SIZE, 0, 0)), None);
        assert!(!chunk.set(IVec3::new(0, SIZE, 0), BlockType::Stone));
    }

    #[test]
    fn set_puis_get_ne_touche_que_la_case_visee() {
        let mut chunk = Chunk::filled(BlockType::Air);

        assert!(chunk.set(IVec3::new(3, 5, 7), BlockType::Grass));

        assert_eq!(chunk.get(IVec3::new(3, 5, 7)), Some(BlockType::Grass));
        assert_eq!(chunk.get(IVec3::new(4, 5, 7)), Some(BlockType::Air));
        assert_eq!(chunk.get(IVec3::new(3, 5, 6)), Some(BlockType::Air));
    }

    #[test]
    fn chaque_position_a_un_index_unique() {
        let mut seen = vec![false; CHUNK_VOLUME];
        for z in 0..SIZE {
            for y in 0..SIZE {
                for x in 0..SIZE {
                    let i = index(IVec3::new(x, y, z)).expect("position locale valide");
                    assert!(!seen[i], "index en double : {i}");
                    seen[i] = true;
                }
            }
        }
        assert!(seen.iter().all(|&s| s));
    }
}
