//! Sauvegarde du monde : fichier JSON versionné, un enregistrement par chunk.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use glam::IVec3;
use serde::{Deserialize, Serialize};

use crate::camera::Camera;
use crate::crafting::Inventory;
use crate::voxel::block::BlockType;
use crate::voxel::chunk::{Chunk, CHUNK_VOLUME};
use crate::voxel::world::World;
use crate::weapons::WeaponSaveState;

/// Version du format de fichier. À incrémenter à chaque changement incompatible.
pub const FORMAT_VERSION: u32 = 10;

const MAX_SAVED_PLAYER_COORD: f32 = 1_000_000.0;
const MAX_PLAYER_HEALTH: u8 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerSaveState {
    pub position: [f32; 3],
    pub health: u8,
    #[serde(default)]
    pub yaw: f32,
    #[serde(default)]
    pub pitch: f32,
    #[serde(default)]
    pub day_phase: f32,
    #[serde(default)]
    pub weapons: Option<WeaponSaveState>,
}

impl PlayerSaveState {
    fn is_valid(self) -> bool {
        self.health > 0
            && self.health <= MAX_PLAYER_HEALTH
            && Camera::is_valid_rotation(self.yaw, self.pitch)
            && self.day_phase.is_finite()
            && (0.0..1.0).contains(&self.day_phase)
            && self.weapons.is_none_or(WeaponSaveState::is_valid)
            && self.position.iter().all(|coordinate| {
                coordinate.is_finite() && coordinate.abs() <= MAX_SAVED_PLAYER_COORD
            })
    }
}

/// Fichier complet. Les chunks sont triés par coordonnée : un même monde donne le même texte.
#[derive(Serialize, Deserialize)]
struct SaveFile {
    version: u32,
    chunks: Vec<SavedChunk>,
    #[serde(default)]
    inventory: Inventory,
    #[serde(default)]
    defeated_mobs: Vec<u32>,
    #[serde(default)]
    player: Option<PlayerSaveState>,
}

/// Un chunk : sa coordonnée, puis ses blocs dans l'ordre de l'index plat de `Chunk`.
#[derive(Serialize, Deserialize)]
struct SavedChunk {
    coord: [i32; 3],
    blocks: Vec<BlockType>,
}

/// Stockage temporaire d'un chunk évincé du monde chargé.
#[derive(Serialize, Deserialize)]
struct CachedChunk {
    coord: [i32; 3],
    blocks: Vec<BlockType>,
}

/// Stocke un chunk dans `directory` afin de libérer sa mémoire pendant l'exploration.
pub fn cache_chunk(directory: &Path, coord: IVec3, chunk: &Chunk) -> Result<(), SaveError> {
    fs::create_dir_all(directory)?;
    let path = cached_chunk_path(directory, coord);
    let temp = path.with_extension("tmp");
    let text = serde_json::to_string(&CachedChunk {
        coord: coord.to_array(),
        blocks: chunk.blocks().to_vec(),
    })?;
    fs::write(&temp, text)?;
    if path.exists() {
        fs::remove_file(&path)?;
    }
    fs::rename(temp, path)?;
    Ok(())
}

/// Lit un chunk évincé. Un fichier mal formé ou associé à une autre coordonnée est refusé.
pub fn load_cached_chunk(directory: &Path, coord: IVec3) -> Result<Option<Chunk>, SaveError> {
    let path = cached_chunk_path(directory, coord);
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path)?;
    let cached: CachedChunk = serde_json::from_str(&text)?;
    if cached.coord != coord.to_array() {
        return Err(SaveError::Invalid(format!(
            "coordonnée incorrecte dans le cache du chunk {coord}"
        )));
    }
    let chunk = Chunk::from_blocks(cached.blocks).ok_or_else(|| {
        SaveError::Invalid(format!("chunk {coord} en cache de taille incorrecte"))
    })?;
    fs::remove_file(path)?;
    Ok(Some(chunk))
}

fn cached_chunk_path(directory: &Path, coord: IVec3) -> std::path::PathBuf {
    directory.join(format!("{}_{}_{}.json", coord.x, coord.y, coord.z))
}

fn cached_chunks(directory: &Path) -> Result<Vec<CachedChunk>, SaveError> {
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut chunks = Vec::new();
    let mut coords = HashSet::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_file() && entry.path().extension().is_some_and(|ext| ext == "json")
        {
            let text = fs::read_to_string(entry.path())?;
            let cached: CachedChunk = serde_json::from_str(&text)?;
            let coord = IVec3::from_array(cached.coord);
            if entry.file_name().to_string_lossy()
                != format!("{}_{}_{}.json", coord.x, coord.y, coord.z)
            {
                return Err(SaveError::Invalid(format!(
                    "nom incorrect pour le chunk en cache {coord}"
                )));
            }
            if !coords.insert(cached.coord) {
                return Err(SaveError::Invalid(format!(
                    "chunk {coord} en double dans le cache"
                )));
            }
            if cached.blocks.len() != CHUNK_VOLUME {
                return Err(SaveError::Invalid(format!(
                    "chunk {coord} en cache de taille incorrecte"
                )));
            }
            chunks.push(cached);
        }
    }
    Ok(chunks)
}

pub(crate) fn clear_chunk_cache(directory: &Path) -> Result<(), SaveError> {
    if directory.exists() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                fs::remove_file(entry.path())?;
            }
        }
    }
    Ok(())
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
#[cfg(test)]
fn world_to_json(world: &World) -> Result<String, SaveError> {
    world_and_inventory_to_json(world, &Inventory::default())
}

/// Sérialise le monde et l'inventaire dans le format versionné courant.
#[cfg(test)]
pub fn world_and_inventory_to_json(
    world: &World,
    inventory: &Inventory,
) -> Result<String, SaveError> {
    world_inventory_and_mobs_to_json(world, inventory, &[])
}

/// Sérialise le monde, l'inventaire et les ennemis vaincus.
#[cfg(test)]
pub fn world_inventory_and_mobs_to_json(
    world: &World,
    inventory: &Inventory,
    defeated_mobs: &[u32],
) -> Result<String, SaveError> {
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
        inventory: inventory.clone(),
        defeated_mobs: defeated_mobs.to_vec(),
        player: None,
    };
    Ok(serde_json::to_string(&file)?)
}

/// Reconstruit le monde depuis du JSON. Refuse une autre version ou un chunk mal formé.
#[cfg(test)]
fn world_from_json(text: &str) -> Result<World, SaveError> {
    world_and_inventory_from_json(text).map(|(world, _)| world)
}

/// Reconstruit le monde et l'inventaire. Les sauvegardes version 1 chargent un inventaire vide.
#[cfg(test)]
pub fn world_and_inventory_from_json(text: &str) -> Result<(World, Inventory), SaveError> {
    world_inventory_and_mobs_from_json(text).map(|(world, inventory, _)| (world, inventory))
}

/// Reconstruit l'état de jeu. Les anciennes sauvegardes n'ont pas d'ennemis vaincus.
#[cfg(test)]
pub fn world_inventory_and_mobs_from_json(
    text: &str,
) -> Result<(World, Inventory, Vec<u32>), SaveError> {
    let (world, inventory, defeated, _) = world_inventory_mobs_and_player_from_json(text)?;
    Ok((world, inventory, defeated))
}

/// Reconstruit l'état de jeu avec la position et la santé, quand elles sont présentes.
pub fn world_inventory_mobs_and_player_from_json(
    text: &str,
) -> Result<(World, Inventory, Vec<u32>, Option<PlayerSaveState>), SaveError> {
    let file: SaveFile = serde_json::from_str(text)?;
    if !(1..=FORMAT_VERSION).contains(&file.version) {
        return Err(SaveError::UnsupportedVersion(file.version));
    }

    let inventory = file.inventory;
    if !inventory.is_valid() {
        return Err(SaveError::Invalid(
            "quantité d'inventaire supérieure à la limite par pile".to_owned(),
        ));
    }
    let mut ids = HashSet::new();
    if file.defeated_mobs.iter().any(|id| !ids.insert(*id)) {
        return Err(SaveError::Invalid(
            "identifiant d'ennemi vaincu en double".to_owned(),
        ));
    }
    if file.player.is_some_and(|player| !player.is_valid()) {
        return Err(SaveError::Invalid(
            "position ou santé du joueur hors limites".to_owned(),
        ));
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
    Ok((world, inventory, file.defeated_mobs, file.player))
}

/// Écrit le monde dans `path`. Passe par un fichier temporaire pour ne jamais laisser une sauvegarde à moitié écrite.
#[cfg(test)]
fn save_world(path: &Path, world: &World) -> Result<(), SaveError> {
    save_game(path, world, &Inventory::default())
}

/// Écrit le monde et l'inventaire dans `path` en passant par un fichier temporaire.
#[cfg(test)]
pub fn save_game(path: &Path, world: &World, inventory: &Inventory) -> Result<(), SaveError> {
    save_game_state(path, world, inventory, &[])
}

/// Écrit le monde, l'inventaire et la progression des ennemis.
#[cfg(test)]
pub fn save_game_state(
    path: &Path,
    world: &World,
    inventory: &Inventory,
    defeated_mobs: &[u32],
) -> Result<(), SaveError> {
    let text = world_inventory_and_mobs_to_json(world, inventory, defeated_mobs)?;
    let temp = path.with_extension("tmp");
    fs::write(&temp, text)?;
    fs::rename(&temp, path)?;
    Ok(())
}

/// Sauvegarde le monde chargé et fusionne les chunks évincés dans le fichier principal.
pub fn save_game_state_with_cache(
    path: &Path,
    cache_directory: &Path,
    world: &World,
    inventory: &Inventory,
    defeated_mobs: &[u32],
    player: PlayerSaveState,
) -> Result<(), SaveError> {
    if !player.is_valid() {
        return Err(SaveError::Invalid(
            "position ou santé du joueur hors limites".to_owned(),
        ));
    }
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
    let loaded_coords: HashSet<_> = chunks.iter().map(|chunk| chunk.coord).collect();
    chunks.extend(
        cached_chunks(cache_directory)?
            .into_iter()
            .filter(|chunk| !loaded_coords.contains(&chunk.coord))
            .map(|chunk| SavedChunk {
                coord: chunk.coord,
                blocks: chunk.blocks,
            }),
    );
    chunks.sort_by_key(|saved| saved.coord);

    let file = SaveFile {
        version: FORMAT_VERSION,
        chunks,
        inventory: inventory.clone(),
        defeated_mobs: defeated_mobs.to_vec(),
        player: Some(player),
    };
    let text = serde_json::to_string(&file)?;
    let temp = path.with_extension("tmp");
    fs::write(&temp, text)?;
    fs::rename(&temp, path)?;
    clear_chunk_cache(cache_directory)?;
    Ok(())
}

/// Lit le monde écrit par `save_world`.
#[cfg(test)]
fn load_world(path: &Path) -> Result<World, SaveError> {
    load_game(path).map(|(world, _)| world)
}

/// Lit le monde et l'inventaire écrits par `save_game`.
#[cfg(test)]
pub fn load_game(path: &Path) -> Result<(World, Inventory), SaveError> {
    load_game_state(path).map(|(world, inventory, _)| (world, inventory))
}

/// Lit le monde, l'inventaire et les ennemis vaincus.
#[cfg(test)]
pub fn load_game_state(path: &Path) -> Result<(World, Inventory, Vec<u32>), SaveError> {
    let text = fs::read_to_string(path)?;
    world_inventory_and_mobs_from_json(&text)
}

/// Charge la sauvegarde et réintègre les chunks évincés après la dernière sauvegarde.
pub fn load_game_state_with_cache(
    path: &Path,
    cache_directory: &Path,
) -> Result<(World, Inventory, Vec<u32>, Option<PlayerSaveState>), SaveError> {
    let text = fs::read_to_string(path)?;
    let (mut world, inventory, defeated, player) =
        world_inventory_mobs_and_player_from_json(&text)?;
    let cached = cached_chunks(cache_directory)?;
    for saved in cached {
        let coord = IVec3::from_array(saved.coord);
        let chunk = Chunk::from_blocks(saved.blocks).ok_or_else(|| {
            SaveError::Invalid(format!("chunk {coord} en cache de taille incorrecte"))
        })?;
        world.insert_chunk(coord, chunk);
    }
    clear_chunk_cache(cache_directory)?;
    Ok((world, inventory, defeated, player))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::generate_area;

    fn test_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("voxel_frontier_{name}_{}.json", std::process::id()))
    }

    fn test_cache_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "voxel_frontier_{name}_{}_chunks",
            std::process::id()
        ))
    }

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

    #[test]
    fn l_inventaire_est_persiste_avec_le_monde() {
        let world = generate_area(7, 1);
        let mut inventory = Inventory::default();
        inventory.add(BlockType::Dirt, 12);
        inventory.add(BlockType::Brick, 4);
        inventory.add(BlockType::Wood, 7);
        inventory.add(BlockType::Leaves, 5);
        inventory.add(BlockType::Planks, 3);

        let text = world_and_inventory_to_json(&world, &inventory).unwrap();
        let (loaded_world, loaded_inventory) = world_and_inventory_from_json(&text).unwrap();

        assert_eq!(loaded_world, world);
        assert_eq!(loaded_inventory, inventory);
    }

    #[test]
    fn l_inventaire_est_persiste_dans_le_fichier_de_sauvegarde() {
        let path = std::env::temp_dir().join(format!(
            "voxel_frontier_inventory_test_{}.json",
            std::process::id()
        ));
        let world = generate_area(11, 1);
        let mut inventory = Inventory::default();
        inventory.add(BlockType::Stone, 7);
        inventory.add(BlockType::Brick, 2);

        save_game(&path, &world, &inventory).unwrap();
        let loaded = load_game(&path);
        let _ = fs::remove_file(&path);

        let (loaded_world, loaded_inventory) = loaded.unwrap();
        assert_eq!(loaded_world, world);
        assert_eq!(loaded_inventory, inventory);
    }

    #[test]
    fn une_sauvegarde_v1_est_chargee_avec_inventaire_vide() {
        let text = r#"{"version":1,"chunks":[]}"#;

        let (world, inventory) = world_and_inventory_from_json(text).unwrap();

        assert_eq!(world, World::new());
        assert_eq!(inventory, Inventory::default());
    }

    #[test]
    fn une_sauvegarde_v2_migre_les_quatre_anciennes_piles() {
        let text = r#"{"version":2,"chunks":[],"inventory":{"counts":[1,2,3,4]}}"#;

        let (_, inventory) = world_and_inventory_from_json(text).unwrap();

        assert_eq!(inventory.count(BlockType::Grass), 1);
        assert_eq!(inventory.count(BlockType::Dirt), 2);
        assert_eq!(inventory.count(BlockType::Stone), 3);
        assert_eq!(inventory.count(BlockType::Brick), 4);
        assert_eq!(inventory.count(BlockType::Sand), 0);
        assert_eq!(inventory.count(BlockType::Snow), 0);
    }

    #[test]
    fn un_chunk_evince_est_recharge_puis_retire_du_cache() {
        let directory = test_cache_path("chunk_cache_roundtrip");
        let _ = fs::remove_dir_all(&directory);
        let coord = IVec3::new(-3, 1, 7);
        let chunk = Chunk::filled(BlockType::Snow);

        cache_chunk(&directory, coord, &chunk).unwrap();
        assert_eq!(load_cached_chunk(&directory, coord).unwrap(), Some(chunk));
        assert_eq!(load_cached_chunk(&directory, coord).unwrap(), None);

        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn la_sauvegarde_fusionne_les_chunks_evinces_et_vide_le_cache() {
        let path = test_path("save_with_chunk_cache");
        let directory = test_cache_path("save_with_chunk_cache");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(&directory);
        let coord = IVec3::new(-2, 0, 3);
        let mut world = World::new();
        world.insert_chunk(IVec3::ZERO, Chunk::filled(BlockType::Grass));
        cache_chunk(&directory, coord, &Chunk::filled(BlockType::Dirt)).unwrap();

        let player = PlayerSaveState {
            position: [-14.5, 23.0, 49.25],
            health: 7,
            yaw: 2.4,
            pitch: -0.35,
            day_phase: 0.42,
            weapons: Some(crate::weapons::WeaponInventory::default().save_state()),
        };
        save_game_state_with_cache(
            &path,
            &directory,
            &world,
            &Inventory::default(),
            &[1],
            player,
        )
        .unwrap();
        let text = fs::read_to_string(&path).unwrap();
        let (loaded, _, defeated, loaded_player) =
            world_inventory_mobs_and_player_from_json(&text).unwrap();

        assert_eq!(loaded.chunk(IVec3::ZERO), world.chunk(IVec3::ZERO));
        assert_eq!(
            loaded.chunk(coord).and_then(|chunk| chunk.get(IVec3::ZERO)),
            Some(BlockType::Dirt)
        );
        assert_eq!(defeated, vec![1]);
        assert_eq!(loaded_player, Some(player));
        assert!(cached_chunks(&directory).unwrap().is_empty());

        let _ = fs::remove_file(path);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn un_cache_reintegre_les_modifications_posterieures_a_la_sauvegarde() {
        let path = test_path("load_with_chunk_cache");
        let directory = test_cache_path("load_with_chunk_cache");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(&directory);
        let coord = IVec3::new(1, 0, -1);
        let mut saved_world = World::new();
        saved_world.insert_chunk(coord, Chunk::filled(BlockType::Grass));
        save_game_state(&path, &saved_world, &Inventory::default(), &[]).unwrap();
        cache_chunk(&directory, coord, &Chunk::filled(BlockType::Stone)).unwrap();

        let (loaded, _, _, player) = load_game_state_with_cache(&path, &directory).unwrap();

        assert_eq!(
            loaded.chunk(coord).and_then(|chunk| chunk.get(IVec3::ZERO)),
            Some(BlockType::Stone)
        );
        assert_eq!(player, None);
        assert!(cached_chunks(&directory).unwrap().is_empty());

        let _ = fs::remove_file(path);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn une_sauvegarde_v3_preserve_six_piles_et_initialise_la_progression() {
        let text = r#"{"version":3,"chunks":[],"inventory":{"counts":[1,2,3,4,5,6]}}"#;

        let (_, inventory, defeated) = world_inventory_and_mobs_from_json(text).unwrap();

        assert_eq!(inventory.count(BlockType::Grass), 1);
        assert_eq!(inventory.count(BlockType::Dirt), 2);
        assert_eq!(inventory.count(BlockType::Stone), 3);
        assert_eq!(inventory.count(BlockType::Brick), 4);
        assert_eq!(inventory.count(BlockType::Sand), 5);
        assert_eq!(inventory.count(BlockType::Snow), 6);
        assert!(defeated.is_empty());
    }

    #[test]
    fn les_sauvegardes_v4_migrent_sans_position_joueur() {
        let text =
            r#"{"version":4,"chunks":[],"inventory":{"counts":[0,0,0,0,0,0]},"defeated_mobs":[2]}"#;

        let (_, inventory, defeated, player) =
            world_inventory_mobs_and_player_from_json(text).unwrap();

        assert_eq!(inventory, Inventory::default());
        assert_eq!(defeated, vec![2]);
        assert_eq!(player, None);
    }

    #[test]
    fn une_sauvegarde_v5_migre_vers_huit_piles_sans_perdre_le_joueur() {
        let text = r#"{"version":5,"chunks":[],"inventory":{"counts":[1,2,3,4,5,6]},"player":{"position":[1,20,3],"health":8}}"#;

        let (_, inventory, _, player) = world_inventory_mobs_and_player_from_json(text).unwrap();

        assert_eq!(inventory.count(BlockType::Snow), 6);
        assert_eq!(inventory.count(BlockType::Wood), 0);
        assert_eq!(inventory.count(BlockType::Leaves), 0);
        assert_eq!(
            player,
            Some(PlayerSaveState {
                position: [1.0, 20.0, 3.0],
                health: 8,
                yaw: 0.0,
                pitch: 0.0,
                day_phase: 0.0,
                weapons: None,
            })
        );
    }

    #[test]
    fn une_sauvegarde_v6_preserve_les_huit_piles_et_initialise_les_planches() {
        let text = r#"{"version":6,"chunks":[],"inventory":{"counts":[1,2,3,4,5,6,7,8]},"player":{"position":[1,20,3],"health":8}}"#;

        let (_, inventory, _, player) = world_inventory_mobs_and_player_from_json(text).unwrap();

        assert_eq!(inventory.count(BlockType::Wood), 7);
        assert_eq!(inventory.count(BlockType::Leaves), 8);
        assert_eq!(inventory.count(BlockType::Planks), 0);
        assert_eq!(player.unwrap().health, 8);
    }

    #[test]
    fn une_sauvegarde_v7_preserve_les_neuf_piles_et_initialise_le_cycle_jour_nuit() {
        let text = r#"{"version":7,"chunks":[],"inventory":{"counts":[1,2,3,4,5,6,7,8,9]},"player":{"position":[1,20,3],"health":8}}"#;

        let (_, inventory, _, player) = world_inventory_mobs_and_player_from_json(text).unwrap();

        assert_eq!(inventory.count(BlockType::Planks), 9);
        assert_eq!(player.unwrap().day_phase, 0.0);
    }

    #[test]
    fn une_sauvegarde_v8_preserve_le_cycle_et_initialise_l_orientation() {
        let text = r#"{"version":8,"chunks":[],"player":{"position":[1,20,3],"health":8,"day_phase":0.4}}"#;

        let (_, _, _, player) = world_inventory_mobs_and_player_from_json(text).unwrap();
        let player = player.unwrap();

        assert_eq!(player.day_phase, 0.4);
        assert_eq!(player.yaw, 0.0);
        assert_eq!(player.pitch, 0.0);
    }

    #[test]
    fn une_sauvegarde_v9_migre_sans_munitions() {
        let text = r#"{"version":9,"chunks":[],"player":{"position":[1,20,3],"health":8,"yaw":0.4,"pitch":-0.2,"day_phase":0.6}}"#;

        let (_, _, _, player) = world_inventory_mobs_and_player_from_json(text).unwrap();
        let player = player.unwrap();

        assert_eq!(player.yaw, 0.4);
        assert_eq!(player.weapons, None);
    }

    #[test]
    fn une_sauvegarde_refuse_des_munitions_au_dela_des_limites() {
        let text = r#"{"version":10,"chunks":[],"player":{"position":[0,20,0],"health":10,"weapons":{"selected_slot":1,"magazines":[{"loaded":12,"reserve":60},{"loaded":31,"reserve":120},{"loaded":5,"reserve":30}]}}}"#;

        assert!(matches!(
            world_inventory_mobs_and_player_from_json(text),
            Err(SaveError::Invalid(_))
        ));
    }

    #[test]
    fn une_position_ou_une_sante_joueur_invalide_est_refusee() {
        let invalid_position =
            r#"{"version":5,"chunks":[],"player":{"position":[1000001,0,0],"health":10}}"#;
        let invalid_health =
            r#"{"version":5,"chunks":[],"player":{"position":[0,20,0],"health":11}}"#;
        let invalid_day_phase = r#"{"version":8,"chunks":[],"player":{"position":[0,20,0],"health":10,"day_phase":1.0}}"#;
        let invalid_yaw = r#"{"version":9,"chunks":[],"player":{"position":[0,20,0],"health":10,"yaw":6.2831855}}"#;
        let invalid_pitch =
            r#"{"version":9,"chunks":[],"player":{"position":[0,20,0],"health":10,"pitch":2.0}}"#;

        assert!(matches!(
            world_inventory_mobs_and_player_from_json(invalid_position),
            Err(SaveError::Invalid(_))
        ));
        assert!(matches!(
            world_inventory_mobs_and_player_from_json(invalid_health),
            Err(SaveError::Invalid(_))
        ));
        assert!(matches!(
            world_inventory_mobs_and_player_from_json(invalid_day_phase),
            Err(SaveError::Invalid(_))
        ));
        assert!(matches!(
            world_inventory_mobs_and_player_from_json(invalid_yaw),
            Err(SaveError::Invalid(_))
        ));
        assert!(matches!(
            world_inventory_mobs_and_player_from_json(invalid_pitch),
            Err(SaveError::Invalid(_))
        ));
    }

    #[test]
    fn les_ennemis_vaincus_sont_persistes_et_les_ids_dupliques_refuses() {
        let world = generate_area(3, 1);
        let mut inventory = Inventory::default();
        inventory.add(BlockType::Brick, 2);

        let text = world_inventory_and_mobs_to_json(&world, &inventory, &[2, 5]).unwrap();
        let (loaded_world, loaded_inventory, defeated) =
            world_inventory_and_mobs_from_json(&text).unwrap();
        assert_eq!(loaded_world, world);
        assert_eq!(loaded_inventory, inventory);
        assert_eq!(defeated, vec![2, 5]);

        let duplicate = r#"{"version":4,"chunks":[],"inventory":{"counts":[0,0,0,0,0,0]},"defeated_mobs":[2,2]}"#;
        assert!(matches!(
            world_inventory_and_mobs_from_json(duplicate),
            Err(SaveError::Invalid(_))
        ));
    }

    #[test]
    fn un_inventaire_corrompu_est_refuse() {
        let text = r#"{"version":2,"chunks":[],"inventory":{"counts":[1000,0,0,0]}}"#;

        assert!(matches!(
            world_and_inventory_from_json(text),
            Err(SaveError::Invalid(_))
        ));
    }
}
