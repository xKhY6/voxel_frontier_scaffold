//! Sauvegarde du monde : fichier JSON versionné, un enregistrement par chunk.

use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use glam::IVec3;
use serde::{Deserialize, Serialize};

use crate::voxel::block::BlockType;
use crate::voxel::chunk::Chunk;
use crate::voxel::world::World;

/// Version du format de fichier. À incrémenter à chaque changement incompatible.
pub const FORMAT_VERSION: u32 = 1;

/// Fichier complet. Les chunks sont triés par coordonnée : un même monde donne le même texte.
#[derive(Serialize, Deserialize)]
struct SaveFile {
    version: u32,
    chunks: Vec<SavedChunk>,
}

/// Un chunk : sa coordonnée, puis ses blocs dans l'ordre de l'index plat de `Chunk`.
#[derive(Serialize, Deserialize)]
struct SavedChunk {
    coord: [i32; 3],
    blocks: Vec<BlockType>,
}

/// Erreur de sauvegarde ou de chargement.
#[derive(Debug)]
pub enum SaveError {
    Io(io::Error),
    Json(serde_json::Error),
    /// Version lue différente de `FORMAT_VERSION`.
    UnsupportedVersion(u32),
    /// JSON valide mais contenu incohérent : taille de chunk fausse ou coordonnée en double.
    Invalid(String),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::Io(err) => write!(f, "fichier inaccessible : {err}"),
            SaveError::Json(err) => write!(f, "JSON invalide : {err}"),
            SaveError::UnsupportedVersion(version) => write!(
                f,
                "version de format {} non prise en charge (attendue : {})",
                version, FORMAT_VERSION
            ),
            SaveError::Invalid(message) => write!(f, "sauvegarde invalide : {message}"),
        }
    }
}

impl Error for SaveError {}

impl From<io::Error> for SaveError {
    fn from(err: io::Error) -> Self {
        SaveError::Io(err)
    }
}

impl From<serde_json::Error> for SaveError {
    fn from(err: serde_json::Error) -> Self {
        SaveError::Json(err)
    }
}

/// Sérialise le monde en JSON. Seuls les chunks chargés sont écrits.
pub fn world_to_json(world: &World) -> Result<String, SaveError> {
    let mut chunks: Vec<SavedChunk> = world
        .chunk_coords()
        .into_iter()
        .filter_map(|coord| {
            let chunk = world.chunk(coord)?;
            Some(SavedChunk {
                coord: coord.to_array(),
                blocks: chunk.blocks().to_vec(),
            })
        })
        .collect();
    chunks.sort_by_key(|saved| saved.coord);

    let file = SaveFile {
        version: FORMAT_VERSION,
        chunks,
    };
    Ok(serde_json::to_string(&file)?)
}

/// Reconstruit le monde depuis du JSON. Refuse une autre version ou un chunk mal formé.
pub fn world_from_json(text: &str) -> Result<World, SaveError> {
    let file: SaveFile = serde_json::from_str(text)?;
    if file.version != FORMAT_VERSION {
        return Err(SaveError::UnsupportedVersion(file.version));
    }

    let mut world = World::new();
    for saved in file.chunks {
        let coord = IVec3::from_array(saved.coord);
        if world.chunk(coord).is_some() {
            return Err(SaveError::Invalid(format!("chunk {coord} en double")));
        }
        let chunk = Chunk::from_blocks(saved.blocks)
            .ok_or_else(|| SaveError::Invalid(format!("chunk {coord} de taille incorrecte")))?;
        world.insert_chunk(coord, chunk);
    }
    Ok(world)
}

/// Écrit le monde dans `path`. Passe par un fichier temporaire pour ne jamais laisser une sauvegarde à moitié écrite.
pub fn save_world(path: &Path, world: &World) -> Result<(), SaveError> {
    let text = world_to_json(world)?;
    let temp = path.with_extension("tmp");
    fs::write(&temp, text)?;
    fs::rename(&temp, path)?;
    Ok(())
}

/// Lit le monde écrit par `save_world`.
pub fn load_world(path: &Path) -> Result<World, SaveError> {
    let text = fs::read_to_string(path)?;
    world_from_json(&text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::generate_area;

    #[test]
    fn l_aller_retour_json_redonne_le_meme_monde() {
        let world = generate_area(7, 1);

        let text = world_to_json(&world).unwrap();

        assert_eq!(world_from_json(&text).unwrap(), world);
    }

    #[test]
    fn l_aller_retour_fichier_redonne_le_meme_monde() {
        let path =
            std::env::temp_dir().join(format!("voxel_frontier_test_{}.json", std::process::id()));
        let world = generate_area(7, 1);

        save_world(&path, &world).unwrap();
        let loaded = load_world(&path);
        let _ = fs::remove_file(&path);

        assert_eq!(loaded.unwrap(), world);
    }

    #[test]
    fn une_version_inconnue_est_refusee() {
        let text = r#"{"version":999,"chunks":[]}"#;

        assert!(matches!(
            world_from_json(text),
            Err(SaveError::UnsupportedVersion(999))
        ));
    }

    #[test]
    fn un_chunk_de_taille_incorrecte_est_refuse() {
        let text = r#"{"version":1,"chunks":[{"coord":[0,0,0],"blocks":["Stone"]}]}"#;

        assert!(matches!(world_from_json(text), Err(SaveError::Invalid(_))));
    }

    #[test]
    fn un_json_invalide_est_refuse() {
        assert!(matches!(
            world_from_json("pas du json"),
            Err(SaveError::Json(_))
        ));
    }
}
