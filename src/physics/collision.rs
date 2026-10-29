//! Collisions AABB : une boîte alignée sur les axes ne traverse pas les blocs solides.

use glam::{IVec3, Vec3};

use crate::voxel::world::World;

/// Boîte alignée sur les axes, en coordonnées monde.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn translated(self, delta: Vec3) -> Self {
        Self {
            min: self.min + delta,
            max: self.max + delta,
        }
    }

    /// Vrai si la boîte chevauche strictement la cellule `cell` ; un contact de face ne compte pas.
    pub fn overlaps_cell(&self, cell: IVec3) -> bool {
        let cell_min = cell.as_vec3();
        let cell_max = cell_min + Vec3::ONE;
        (0..3).all(|axis| cell_min[axis] < self.max[axis] && cell_max[axis] > self.min[axis])
    }
}

/// Déplace `aabb` de `delta` axe par axe (X, Y, Z) et retourne le déplacement effectif.
/// Un axe bloqué n'empêche pas les autres : la boîte glisse le long des murs.
pub fn move_aabb(world: &World, aabb: Aabb, delta: Vec3) -> Vec3 {
    let mut current = aabb;
    let mut moved = Vec3::ZERO;
    for axis in 0..3 {
        let d = delta[axis];
        if d == 0.0 {
            continue;
        }
        let step = axis_vec(axis, allowed_step(world, current, axis, d));
        current = current.translated(step);
        moved += step;
    }
    moved
}

/// Vecteur dont la seule composante non nulle est `value` sur l'axe `axis`.
fn axis_vec(axis: usize, value: f32) -> Vec3 {
    let mut v = Vec3::ZERO;
    v[axis] = value;
    v
}

/// Boîte couvrant `aabb` et sa position après un déplacement `d` sur `axis`.
fn swept(aabb: Aabb, axis: usize, d: f32) -> Aabb {
    let moved = aabb.translated(axis_vec(axis, d));
    Aabb {
        min: aabb.min.min(moved.min),
        max: aabb.max.max(moved.max),
    }
}

/// Cellules de bloc qui chevauchent strictement la boîte.
fn covered_cells(aabb: Aabb) -> Vec<IVec3> {
    let lo = aabb.min.floor().as_ivec3();
    let hi = aabb.max.ceil().as_ivec3() - IVec3::ONE;
    let mut cells = Vec::new();
    for x in lo.x..=hi.x {
        for y in lo.y..=hi.y {
            for z in lo.z..=hi.z {
                cells.push(IVec3::new(x, y, z));
            }
        }
    }
    cells
}

/// Tolérance sur les contacts : une face enfoncée de moins de cette valeur (erreur d'arrondi) est posée.
const CONTACT_TOLERANCE: f32 = 1e-4;

/// Déplacement autorisé sur `axis` : même signe que `d`, module au plus `|d|`.
fn allowed_step(world: &World, aabb: Aabb, axis: usize, d: f32) -> f32 {
    let mut allowed = d;
    for cell in covered_cells(swept(aabb, axis, d)) {
        if !world.block_at(cell).is_solid() {
            continue;
        }
        let cell_min = cell.as_vec3()[axis];
        let cell_max = cell_min + 1.0;
        // Un contact légèrement enfoncé (erreur d'arrondi) compte comme posé : sinon la boîte traverse le bloc.
        if d > 0.0 && cell_min >= aabb.max[axis] - CONTACT_TOLERANCE {
            allowed = allowed.min((cell_min - aabb.max[axis]).max(0.0));
        } else if d < 0.0 && cell_max <= aabb.min[axis] + CONTACT_TOLERANCE {
            allowed = allowed.max((cell_max - aabb.min[axis]).min(0.0));
        }
    }
    allowed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voxel::block::BlockType;
    use crate::voxel::chunk::{Chunk, CHUNK_SIZE};

    const EPSILON: f32 = 1e-4;

    /// Chunk (0, 0, 0) avec un sol de pierre en y = 0.
    fn floor_world() -> World {
        let size = CHUNK_SIZE as i32;
        let mut chunk = Chunk::filled(BlockType::Air);
        for x in 0..size {
            for z in 0..size {
                chunk.set(IVec3::new(x, 0, z), BlockType::Stone);
            }
        }
        let mut world = World::new();
        world.insert_chunk(IVec3::ZERO, chunk);
        world
    }

    /// Boîte posée sur le sol, au centre de la colonne (5, 5).
    fn standing_box() -> Aabb {
        Aabb {
            min: Vec3::new(4.7, 1.0, 4.7),
            max: Vec3::new(5.3, 2.8, 5.3),
        }
    }

    fn assert_proche(obtenu: Vec3, attendu: Vec3) {
        assert!(
            (obtenu - attendu).length() < EPSILON,
            "attendu {attendu:?}, obtenu {obtenu:?}"
        );
    }

    #[test]
    fn la_chute_s_arrete_sur_le_sol() {
        let moved = move_aabb(&floor_world(), standing_box(), Vec3::new(0.0, -3.0, 0.0));

        assert_eq!(moved.y, 0.0);
    }

    #[test]
    fn un_pied_a_peine_enfonce_reste_pose_sur_le_sol() {
        let mut aabb = standing_box();
        aabb.min.y -= 1e-7;
        aabb.max.y -= 1e-7;

        let moved = move_aabb(&floor_world(), aabb, Vec3::new(0.0, -0.01, 0.0));

        assert_eq!(moved.y, 0.0);
    }

    #[test]
    fn un_mouvement_libre_n_est_pas_bloque() {
        let delta = Vec3::new(0.5, 0.0, -0.25);

        assert_proche(move_aabb(&floor_world(), standing_box(), delta), delta);
    }

    #[test]
    fn un_mur_bloque_son_axe_sans_figer_les_autres() {
        let mut world = floor_world();
        for y in 1..=2 {
            for z in 4..=5 {
                assert!(world.set_block(IVec3::new(7, y, z), BlockType::Stone));
            }
        }

        let moved = move_aabb(&world, standing_box(), Vec3::new(3.0, 0.0, 0.5));

        // Le mur est en x = 7 : la boîte s'arrete en x = 7 (bord max a 5.3 + 1.7), z continue.
        assert_proche(moved, Vec3::new(1.7, 0.0, 0.5));
    }

    #[test]
    fn un_mur_bloque_aussi_dans_le_sens_negatif() {
        let mut world = floor_world();
        for y in 1..=2 {
            for z in 4..=5 {
                assert!(world.set_block(IVec3::new(3, y, z), BlockType::Stone));
            }
        }

        let moved = move_aabb(&world, standing_box(), Vec3::new(-2.0, 0.0, 0.0));

        assert!((moved.x + 0.7).abs() < EPSILON);
    }

    #[test]
    fn le_contact_de_face_ne_compte_pas_comme_chevauchement() {
        let cube = Aabb {
            min: Vec3::ZERO,
            max: Vec3::ONE,
        };

        assert!(cube.overlaps_cell(IVec3::ZERO));
        assert!(!cube.overlaps_cell(IVec3::new(1, 0, 0)));
        assert!(!cube.overlaps_cell(IVec3::new(-1, 0, 0)));
    }
}
