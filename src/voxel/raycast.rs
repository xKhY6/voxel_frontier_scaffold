//! Rayon de sélection : parcours des cellules de bloc le long d'un rayon (Amanatides et Woo).

use glam::{IVec3, Vec3};

use super::world::World;

/// Bloc solide touché par un rayon et normale de la face touchée.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    pub block: IVec3,
    /// Pointe vers l'origine du rayon : `block + normal` est la cellule libre devant la face.
    pub normal: IVec3,
}

/// Premier bloc solide le long du rayon, sur au plus `max_distance` blocs.
/// `None` si la direction est nulle ou si l'origine est déjà dans un bloc solide.
pub fn raycast(world: &World, origin: Vec3, direction: Vec3, max_distance: f32) -> Option<RayHit> {
    let dir = direction.normalize_or_zero();
    if dir == Vec3::ZERO {
        return None;
    }

    let mut cell = origin.floor().as_ivec3();
    if world.block_at(cell).is_solid() {
        return None;
    }

    let step = [sign(dir.x), sign(dir.y), sign(dir.z)];
    // Distance le long du rayon pour traverser une cellule entière (infinie sur un axe parallèle).
    let t_delta = Vec3::ONE / dir.abs();
    let mut t_max: [f32; 3] = std::array::from_fn(|axis| {
        if step[axis] == 0 {
            return f32::INFINITY;
        }
        let boundary = if step[axis] > 0 {
            cell[axis] as f32 + 1.0
        } else {
            cell[axis] as f32
        };
        (boundary - origin[axis]) / dir[axis]
    });

    loop {
        // Frontière la plus proche ; à égalité, X puis Y puis Z.
        let axis = if t_max[0] <= t_max[1] && t_max[0] <= t_max[2] {
            0
        } else if t_max[1] <= t_max[2] {
            1
        } else {
            2
        };
        if t_max[axis] > max_distance {
            return None;
        }

        cell[axis] += step[axis];
        t_max[axis] += t_delta[axis];

        if world.block_at(cell).is_solid() {
            let mut normal = IVec3::ZERO;
            normal[axis] = -step[axis];
            return Some(RayHit {
                block: cell,
                normal,
            });
        }
    }
}

/// Sens de parcours sur un axe : `1`, `-1`, ou `0` si le rayon est parallèle à l'axe.
fn sign(v: f32) -> i32 {
    if v > 0.0 {
        1
    } else if v < 0.0 {
        -1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voxel::block::BlockType;
    use crate::voxel::chunk::Chunk;
    use crate::voxel::world::split_block_pos;

    /// Monde où seules les cellules données sont solides, dans des chunks chargés.
    fn world_with_stones(stones: &[IVec3]) -> World {
        let mut world = World::new();
        for &cell in stones {
            let (coord, _) = split_block_pos(cell);
            if world.chunk(coord).is_none() {
                world.insert_chunk(coord, Chunk::filled(BlockType::Air));
            }
            assert!(world.set_block(cell, BlockType::Stone));
        }
        world
    }

    #[test]
    fn le_rayon_touche_le_premier_bloc_solide() {
        let world = world_with_stones(&[IVec3::new(0, 0, -3)]);

        let hit = raycast(
            &world,
            Vec3::new(0.5, 0.5, 0.5),
            Vec3::new(0.0, 0.0, -1.0),
            10.0,
        );

        assert_eq!(
            hit,
            Some(RayHit {
                block: IVec3::new(0, 0, -3),
                normal: IVec3::new(0, 0, 1),
            })
        );
    }

    #[test]
    fn la_portee_limite_le_rayon() {
        let world = world_with_stones(&[IVec3::new(0, 0, -3)]);
        let origin = Vec3::new(0.5, 0.5, 0.5);
        let direction = Vec3::new(0.0, 0.0, -1.0);

        // Le bloc commence à 2.5 blocs de l'origine.
        assert_eq!(raycast(&world, origin, direction, 2.0), None);
        assert!(raycast(&world, origin, direction, 3.0).is_some());
    }

    #[test]
    fn les_coordonnees_negatives_sont_gerees() {
        let world = world_with_stones(&[IVec3::new(-1, 0, 0)]);

        let hit = raycast(
            &world,
            Vec3::new(-2.5, 0.5, 0.5),
            Vec3::new(1.0, 0.0, 0.0),
            10.0,
        );

        assert_eq!(
            hit,
            Some(RayHit {
                block: IVec3::new(-1, 0, 0),
                normal: IVec3::new(-1, 0, 0),
            })
        );
    }

    #[test]
    fn le_rayon_vient_de_dessus() {
        let world = world_with_stones(&[IVec3::new(0, 2, 0)]);

        let hit = raycast(
            &world,
            Vec3::new(0.5, 5.5, 0.5),
            Vec3::new(0.0, -1.0, 0.0),
            10.0,
        );

        assert_eq!(
            hit,
            Some(RayHit {
                block: IVec3::new(0, 2, 0),
                normal: IVec3::new(0, 1, 0),
            })
        );
    }

    #[test]
    fn une_origine_dans_un_bloc_solide_ne_touche_rien() {
        let world = world_with_stones(&[IVec3::ZERO]);

        assert_eq!(
            raycast(&world, Vec3::splat(0.5), Vec3::new(1.0, 0.0, 0.0), 10.0),
            None
        );
    }

    #[test]
    fn une_direction_nulle_ne_touche_rien() {
        let world = world_with_stones(&[IVec3::new(1, 0, 0)]);

        assert_eq!(raycast(&world, Vec3::splat(0.5), Vec3::ZERO, 10.0), None);
    }
}
