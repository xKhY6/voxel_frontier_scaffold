//! Génération procédurale déterministe : même graine, même monde.
//!
//! Le relief est un bruit de valeur sur (x, z). La hauteur ne dépend que des coordonnées monde :
//! deux chunks voisins se raccordent sans couture.

use glam::IVec3;

use crate::voxel::block::BlockType;
use crate::voxel::chunk::{Chunk, CHUNK_SIZE};
use crate::voxel::world::World;

/// Hauteur minimale de la surface, en blocs.
const BASE_HEIGHT: f32 = 8.0;

/// Écart maximal ajouté à `BASE_HEIGHT` : la surface reste dans `BASE_HEIGHT..BASE_HEIGHT + HEIGHT_AMPLITUDE`.
const HEIGHT_AMPLITUDE: f32 = 16.0;

/// Taille d'une cellule du bruit de base, en blocs.
const FEATURE_SIZE: f32 = 16.0;

/// Taille caractéristique des zones de biome, en blocs.
const BIOME_SIZE: f32 = 48.0;

/// Épaisseur de la couche de terre sous l'herbe, en blocs.
const DIRT_DEPTH: i32 = 3;

/// Trois chunks verticaux couvrent le relief et les cimes des arbres.
const AREA_LAYERS: i32 = 3;
const TREE_DENSITY: f32 = 0.015;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
enum Biome {
    Plains,
    Desert,
    Tundra,
}

/// Génère le chunk `coord` à partir de la graine. Même graine et même coordonnée donnent le même chunk.
pub fn generate_chunk(seed: u64, coord: IVec3) -> Chunk {
    let size = CHUNK_SIZE as i32;
    let origin = coord * size;
    let mut chunk = Chunk::filled(BlockType::Air);

    for x in 0..size {
        for z in 0..size {
            let world_x = origin.x + x;
            let world_z = origin.z + z;
            let biome = biome_at(seed, world_x, world_z);
            let surface = surface_height(seed, world_x, world_z);
            for y in 0..size {
                let block = block_for_depth(origin.y + y, surface, biome);
                chunk.set(IVec3::new(x, y, z), block);
            }
        }
    }
    place_vegetation(seed, origin, &mut chunk);
    chunk
}

fn place_vegetation(seed: u64, origin: IVec3, chunk: &mut Chunk) {
    let size = CHUNK_SIZE as i32;
    let x_range = origin.x - 2..origin.x + size + 2;
    let z_range = origin.z - 2..origin.z + size + 2;

    for x in x_range.clone() {
        for z in z_range.clone() {
            if !is_tree_candidate(seed, x, z) {
                continue;
            }
            let surface = surface_height(seed, x, z);
            for layer in 0..3 {
                let y = surface + 3 + layer;
                let radius: i32 = if layer == 2 { 1 } else { 2 };
                for dx in -radius..=radius {
                    for dz in -radius..=radius {
                        if dx.abs() == radius && dz.abs() == radius {
                            continue;
                        }
                        set_chunk_block_if_air(
                            chunk,
                            origin,
                            IVec3::new(x + dx, y, z + dz),
                            BlockType::Leaves,
                        );
                    }
                }
            }
        }
    }

    for x in x_range {
        for z in z_range.clone() {
            if !is_tree_candidate(seed, x, z) {
                continue;
            }
            let surface = surface_height(seed, x, z);
            for y in surface + 1..=surface + 4 {
                set_chunk_block_if_air(chunk, origin, IVec3::new(x, y, z), BlockType::Wood);
            }
        }
    }
}

fn set_chunk_block_if_air(chunk: &mut Chunk, origin: IVec3, world_pos: IVec3, block: BlockType) {
    let local = world_pos - origin;
    if chunk.get(local) == Some(BlockType::Air) {
        chunk.set(local, block);
    }
}

fn is_tree_candidate(seed: u64, x: i32, z: i32) -> bool {
    biome_at(seed, x, z) == Biome::Plains
        && lattice_value(seed ^ 0xA076_1D64_78BD_642F, x as i64, z as i64) < TREE_DENSITY
}

/// Génère une zone de chunks : colonnes de `-radius..radius` sur x et z, `AREA_LAYERS` couches en y à partir de 0.
pub fn generate_area(seed: u64, radius: i32) -> World {
    let mut world = World::new();
    for x in -radius..radius {
        for z in -radius..radius {
            for y in 0..AREA_LAYERS {
                let coord = IVec3::new(x, y, z);
                world.insert_chunk(coord, generate_chunk(seed, coord));
            }
        }
    }
    world
}

/// Hauteur de la surface en (x, z) monde.
pub fn surface_height(seed: u64, x: i32, z: i32) -> i32 {
    let fx = x as f32 / FEATURE_SIZE;
    let fz = z as f32 / FEATURE_SIZE;
    // Deux octaves : une forme large (3/4) et des détails plus fins (1/4).
    let noise = 0.75 * value_noise(seed, fx, fz) + 0.25 * value_noise(seed, fx * 4.0, fz * 4.0);
    let (base, amplitude) = match biome_at(seed, x, z) {
        Biome::Plains => (BASE_HEIGHT, HEIGHT_AMPLITUDE),
        Biome::Desert => (5.0, 5.0),
        Biome::Tundra => (18.0, 11.0),
    };
    (base + noise * amplitude) as i32
}

fn biome_at(seed: u64, x: i32, z: i32) -> Biome {
    let value = value_noise(
        seed ^ 0xD6E8_FEB8_6659_FD93,
        x as f32 / BIOME_SIZE,
        z as f32 / BIOME_SIZE,
    );
    if value < 0.32 {
        Biome::Desert
    } else if value > 0.72 {
        Biome::Tundra
    } else {
        Biome::Plains
    }
}

/// Bloc attendu à la hauteur monde `y` d'une colonne dont la surface est `surface`.
/// Le matériau de surface et sa couche dépendent du biome.
fn block_for_depth(y: i32, surface: i32, biome: Biome) -> BlockType {
    if y > surface {
        BlockType::Air
    } else if y == surface {
        match biome {
            Biome::Plains => BlockType::Grass,
            Biome::Desert => BlockType::Sand,
            Biome::Tundra => BlockType::Snow,
        }
    } else if biome == Biome::Desert && y >= surface - 3 {
        BlockType::Sand
    } else if y >= surface - DIRT_DEPTH {
        BlockType::Dirt
    } else {
        BlockType::Stone
    }
}

/// Bruit de valeur 2D : valeurs aux coins de la cellule entière, interpolées avec une courbe lisse.
fn value_noise(seed: u64, x: f32, z: f32) -> f32 {
    let x0 = x.floor();
    let z0 = z.floor();
    let tx = smoothstep(x - x0);
    let tz = smoothstep(z - z0);
    let (ix, iz) = (x0 as i64, z0 as i64);

    let a = lattice_value(seed, ix, iz);
    let b = lattice_value(seed, ix + 1, iz);
    let c = lattice_value(seed, ix, iz + 1);
    let d = lattice_value(seed, ix + 1, iz + 1);
    lerp(lerp(a, b, tx), lerp(c, d, tx), tz)
}

fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Valeur pseudo-aléatoire dans `[0, 1)` attachée à un point du réseau (mélange de bits de type splitmix64).
fn lattice_value(seed: u64, x: i64, z: i64) -> f32 {
    let mut h = seed
        ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (z as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 30;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    // 24 bits de poids fort : tiennent exactement dans la mantisse d'un f32, résultat strictement inférieur à 1.
    (h >> 40) as f32 / (1u64 << 24) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: u64 = 42;

    #[test]
    fn la_meme_graine_donne_le_meme_chunk() {
        let coord = IVec3::new(1, 0, -2);

        assert_eq!(generate_chunk(SEED, coord), generate_chunk(SEED, coord));
    }

    #[test]
    fn une_autre_graine_change_le_relief() {
        let coord = IVec3::ZERO;

        assert_ne!(generate_chunk(1, coord), generate_chunk(2, coord));
    }

    #[test]
    fn le_relief_reste_dans_les_bornes() {
        for x in -200..200 {
            for z in -200..200 {
                let h = surface_height(SEED, x, z);
                assert!(
                    (5..30).contains(&h),
                    "surface hors bornes en ({x}, {z}) : {h}"
                );
            }
        }
    }

    #[test]
    fn les_couches_de_terrain_dependant_du_biome() {
        let surface = 10;

        assert_eq!(
            block_for_depth(surface + 1, surface, Biome::Plains),
            BlockType::Air
        );
        assert_eq!(
            block_for_depth(surface, surface, Biome::Plains),
            BlockType::Grass
        );
        assert_eq!(
            block_for_depth(surface - DIRT_DEPTH, surface, Biome::Plains),
            BlockType::Dirt
        );
        assert_eq!(
            block_for_depth(surface - DIRT_DEPTH - 1, surface, Biome::Plains),
            BlockType::Stone
        );
        assert_eq!(
            block_for_depth(surface - 2, surface, Biome::Desert),
            BlockType::Sand
        );
        assert_eq!(
            block_for_depth(surface, surface, Biome::Tundra),
            BlockType::Snow
        );
    }

    #[test]
    fn la_zone_couvre_les_chunks_attendus() {
        let world = generate_area(SEED, 2);

        assert!(world.chunk(IVec3::new(-2, 0, -2)).is_some());
        assert!(world.chunk(IVec3::new(1, AREA_LAYERS - 1, 1)).is_some());
        assert!(world.chunk(IVec3::new(2, 0, 0)).is_none());
        assert!(world.chunk(IVec3::new(0, AREA_LAYERS, 0)).is_none());
    }

    #[test]
    fn les_chunks_voisins_se_raccordent_sans_couture() {
        // Traverse la frontière des chunks en x = 0 (chunks -1 et 0) et x = 16 (chunks 0 et 1).
        let world = generate_area(SEED, 2);

        for x in -17..17 {
            for z in -1..1 {
                let h = surface_height(SEED, x, z);
                assert_eq!(
                    world.block_at(IVec3::new(x, h, z)),
                    block_for_depth(h, h, biome_at(SEED, x, z)),
                    "x={x} z={z}"
                );
                assert!(
                    matches!(
                        world.block_at(IVec3::new(x, h + 1, z)),
                        BlockType::Air | BlockType::Wood | BlockType::Leaves
                    ),
                    "x={x} z={z}"
                );
            }
        }
    }

    #[test]
    fn les_arbres_sont_deterministes_et_traversent_les_frontieres() {
        let candidate = (-2048_i32..2048)
            .filter(|x| x.rem_euclid(CHUNK_SIZE as i32) == CHUNK_SIZE as i32 - 1)
            .find_map(|x| {
                (-64..64).find_map(|z| {
                    if !is_tree_candidate(SEED, x, z) || is_tree_candidate(SEED, x + 1, z) {
                        return None;
                    }
                    let surface = surface_height(SEED, x, z);
                    let trunk_layer = (surface + 1).div_euclid(CHUNK_SIZE as i32);
                    let leaves_layer = (surface + 3).div_euclid(CHUNK_SIZE as i32);
                    (trunk_layer == leaves_layer).then_some((x, z, surface))
                })
            })
            .expect("un arbre doit se trouver près d'une frontière de chunk");
        let (x, z, surface) = candidate;
        let size = CHUNK_SIZE as i32;
        let trunk_coord = IVec3::new(
            x.div_euclid(size),
            (surface + 1).div_euclid(size),
            z.div_euclid(size),
        );
        let leaves_coord = IVec3::new(
            (x + 1).div_euclid(size),
            (surface + 3).div_euclid(size),
            z.div_euclid(size),
        );
        let trunk = generate_chunk(SEED, trunk_coord);
        let leaves = generate_chunk(SEED, leaves_coord);

        assert_eq!(
            trunk.get(IVec3::new(x, surface + 1, z) - trunk_coord * size),
            Some(BlockType::Wood)
        );
        assert_eq!(
            leaves.get(IVec3::new(x + 1, surface + 3, z) - leaves_coord * size),
            Some(BlockType::Leaves)
        );
        assert_eq!(trunk, generate_chunk(SEED, trunk_coord));
        assert_eq!(leaves, generate_chunk(SEED, leaves_coord));
    }

    #[test]
    fn les_trois_biomes_sont_generes() {
        let biomes: std::collections::HashSet<_> = (-256..256)
            .step_by(4)
            .flat_map(|x| (-256..256).step_by(4).map(move |z| biome_at(SEED, x, z)))
            .collect();

        assert_eq!(biomes.len(), 3);
    }
}
