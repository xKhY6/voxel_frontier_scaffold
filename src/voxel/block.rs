use serde::{Deserialize, Serialize};

/// Type de bloc. `Air` désigne une cellule vide.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum BlockType {
    Air,
    Grass,
    Dirt,
    Stone,
}

impl BlockType {
    /// Vrai si le bloc bloque le joueur et les rayons. Seul `Air` ne l'est pas.
    pub fn is_solid(self) -> bool {
        self != BlockType::Air
    }
}
