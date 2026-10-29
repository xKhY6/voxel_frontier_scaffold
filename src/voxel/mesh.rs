//! Maillage : transforme les faces exposées d'un chunk en triangles.

use glam::{IVec3, Vec3};

use super::block::BlockType;
use super::chunk::CHUNK_SIZE;
use super::world::{ChunkCoord, World};
use crate::lighting::VoxelLighting;

/// Sommet : position monde puis couleur RVB. `repr(C)` : disposition attendue par le GPU.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub normal: [f32; 3],
}

/// Sommets et triangles d'un chunk. Vide si le chunk n'est pas chargé ou n'a aucune face exposée.
#[derive(Debug, Default, PartialEq)]
pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

/// Face d'un bloc unité : direction du voisin, coins relatifs au coin min, facteur d'ombrage.
struct Face {
    normal: [i32; 3],
    corners: [[f32; 3]; 4],
    shade: f32,
}

/// Faces antihoraires vues de l'extérieur (requis par le back-face culling).
const FACES: [Face; 6] = [
    // +X
    Face {
        normal: [1, 0, 0],
        corners: [
            [1.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
        ],
        shade: 0.8,
    },
    // -X
    Face {
        normal: [-1, 0, 0],
        corners: [
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 1.0],
            [0.0, 1.0, 0.0],
        ],
        shade: 0.8,
    },
    // +Y
    Face {
        normal: [0, 1, 0],
        corners: [
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ],
        shade: 1.0,
    },
    // -Y
    Face {
        normal: [0, -1, 0],
        corners: [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
        ],
        shade: 0.5,
    },
    // +Z
    Face {
        normal: [0, 0, 1],
        corners: [
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ],
        shade: 0.65,
    },
    // -Z
    Face {
        normal: [0, 0, -1],
        corners: [
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
        ],
        shade: 0.65,
    },
];

/// Couleur RVB d'un bloc, ou `None` pour l'air (rien à dessiner).
fn block_color(block: BlockType) -> Option<[f32; 3]> {
    match block {
        BlockType::Air => None,
        BlockType::Grass => Some([0.35, 0.7, 0.25]),
        BlockType::Dirt => Some([0.45, 0.3, 0.15]),
        BlockType::Stone => Some([0.5, 0.5, 0.52]),
        BlockType::Brick => Some([0.65, 0.25, 0.18]),
        BlockType::Sand => Some([0.82, 0.74, 0.48]),
        BlockType::Snow => Some([0.88, 0.92, 0.98]),
        BlockType::Wood => Some([0.42, 0.25, 0.1]),
        BlockType::Leaves => Some([0.18, 0.48, 0.16]),
        BlockType::Planks => Some([0.68, 0.48, 0.25]),
    }
}

/// Maillage du chunk `coord`. Une face n'est émise que si le voisin n'est pas solide ; un voisin hors chunk chargé compte comme air.
#[cfg(test)]
pub fn build_chunk_mesh(world: &World, coord: ChunkCoord) -> MeshData {
    build_chunk_mesh_with_lighting(world, coord, &VoxelLighting::calculate(world))
}

/// Construit le maillage en appliquant la lumière précalculée aux faces exposées.
pub fn build_chunk_mesh_with_lighting(
    world: &World,
    coord: ChunkCoord,
    lighting: &VoxelLighting,
) -> MeshData {
    let mut mesh = MeshData::default();
    let Some(chunk) = world.chunk(coord) else {
        return mesh;
    };

    let size = CHUNK_SIZE as i32;
    let origin = coord * size;
    for z in 0..size {
        for y in 0..size {
            for x in 0..size {
                let local = IVec3::new(x, y, z);
                let Some(color) = chunk.get(local).and_then(block_color) else {
                    continue;
                };
                let pos = origin + local;
                for face in &FACES {
                    let neighbor = pos + IVec3::from_array(face.normal);
                    if world.block_at(neighbor).is_solid() {
                        continue;
                    }
                    let light = lighting.factor_at(neighbor);
                    push_face(&mut mesh, pos, face, color.map(|channel| channel * light));
                }
            }
        }
    }
    mesh
}

/// Maillage d'un cube unitaire coloré, utilisé pour les entités mobiles.
pub fn build_entity_cube_mesh(color: [f32; 3]) -> MeshData {
    let mut mesh = MeshData::default();
    for face in &FACES {
        push_face(&mut mesh, IVec3::ZERO, face, color);
    }
    mesh
}

/// Ajoute les deux triangles d'une face du bloc `pos`.
fn push_face(mesh: &mut MeshData, pos: IVec3, face: &Face, color: [f32; 3]) {
    let base = mesh.vertices.len() as u32;
    let shaded = color.map(|c| c * face.shade);
    let origin = pos.as_vec3();
    for corner in face.corners {
        mesh.vertices.push(Vertex {
            position: (origin + Vec3::from_array(corner)).to_array(),
            color: shaded,
            normal: face.normal.map(|component| component as f32),
        });
    }
    mesh.indices
        .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voxel::chunk::Chunk;

    /// Monde ne contenant qu'un chunk (0, 0, 0) rempli de `block`.
    fn single_chunk_world(block: BlockType) -> World {
        let mut world = World::new();
        world.insert_chunk(IVec3::ZERO, Chunk::filled(block));
        world
    }

    #[test]
    fn les_faces_sont_orientees_vers_l_exterieur() {
        for face in &FACES {
            let p: [Vec3; 4] = face.corners.map(Vec3::from_array);
            let normal = (p[1] - p[0]).cross(p[2] - p[0]);
            let outward = Vec3::from_array(face.normal.map(|v| v as f32));

            assert!(
                normal.dot(outward) > 0.0,
                "face mal orientee : {:?}",
                face.corners
            );
            // Les quatre coins sont dans le plan de la face, à un demi-bloc du centre.
            for corner in p {
                let distance = (corner - Vec3::splat(0.5)).dot(outward);
                assert!((distance - 0.5).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn un_bloc_isole_donne_six_faces() {
        let mut chunk = Chunk::filled(BlockType::Air);
        chunk.set(IVec3::new(5, 5, 5), BlockType::Stone);
        let mut world = World::new();
        world.insert_chunk(IVec3::ZERO, chunk);

        let mesh = build_chunk_mesh(&world, IVec3::ZERO);

        assert_eq!(mesh.indices.len(), 36);
        assert_eq!(mesh.vertices.len(), 24);
    }

    #[test]
    fn les_sommets_portent_une_normale_unitaire_par_face() {
        let mut chunk = Chunk::filled(BlockType::Air);
        chunk.set(IVec3::new(5, 5, 5), BlockType::Stone);
        let mut world = World::new();
        world.insert_chunk(IVec3::ZERO, chunk);
        let mesh = build_chunk_mesh(&world, IVec3::ZERO);

        assert!(mesh.vertices.iter().all(|vertex| {
            let normal = Vec3::from_array(vertex.normal);
            (normal.length() - 1.0).abs() < 1e-6
                && vertex
                    .normal
                    .iter()
                    .filter(|component| **component != 0.0)
                    .count()
                    == 1
        }));
    }

    #[test]
    fn les_faces_entre_blocs_solides_ne_sont_pas_emises() {
        // Deux blocs adjacents : 10 faces visibles au lieu de 12.
        let mut chunk = Chunk::filled(BlockType::Air);
        chunk.set(IVec3::new(5, 5, 5), BlockType::Stone);
        chunk.set(IVec3::new(6, 5, 5), BlockType::Stone);
        let mut world = World::new();
        world.insert_chunk(IVec3::ZERO, chunk);

        let mesh = build_chunk_mesh(&world, IVec3::ZERO);

        assert_eq!(mesh.indices.len(), 60);
    }

    #[test]
    fn le_bord_sans_voisin_charge_est_expose() {
        let world = single_chunk_world(BlockType::Stone);

        let mesh = build_chunk_mesh(&world, IVec3::ZERO);

        // Six faces de 16 x 16 blocs sur les bords du chunk.
        assert_eq!(mesh.indices.len(), 6 * CHUNK_SIZE * CHUNK_SIZE * 6);
    }

    #[test]
    fn un_voisin_charge_cache_la_face_de_bord() {
        let mut world = single_chunk_world(BlockType::Stone);
        world.insert_chunk(IVec3::new(1, 0, 0), Chunk::filled(BlockType::Stone));

        let mesh = build_chunk_mesh(&world, IVec3::ZERO);

        // La face +X est cachée par le chunk voisin : cinq faces de 16 x 16 restent.
        assert_eq!(mesh.indices.len(), 5 * CHUNK_SIZE * CHUNK_SIZE * 6);
    }

    #[test]
    fn un_chunk_absent_donne_un_maillage_vide() {
        assert_eq!(
            build_chunk_mesh(&World::new(), IVec3::ZERO),
            MeshData::default()
        );
    }
}
