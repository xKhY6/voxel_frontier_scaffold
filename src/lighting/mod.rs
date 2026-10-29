use std::collections::{HashMap, HashSet, VecDeque};

use glam::{IVec3, Vec3};

use crate::voxel::block::BlockType;
use crate::voxel::chunk::CHUNK_SIZE;
use crate::voxel::world::World;

const MAX_LIGHT: u8 = 15;
const DAY_LENGTH_SECONDS: f32 = 240.0;
const NIGHT_LIGHT: f32 = 0.28;
const NEIGHBORS: [IVec3; 6] = [
    IVec3::X,
    IVec3::NEG_X,
    IVec3::Y,
    IVec3::NEG_Y,
    IVec3::Z,
    IVec3::NEG_Z,
];

#[derive(Clone, Copy, Debug, Default)]
pub struct DayNightCycle {
    elapsed: f32,
}

impl DayNightCycle {
    pub fn from_phase(phase: f32) -> Option<Self> {
        (phase.is_finite() && (0.0..1.0).contains(&phase)).then_some(Self {
            elapsed: phase * DAY_LENGTH_SECONDS,
        })
    }

    pub fn advance(&mut self, seconds: f32) {
        if seconds.is_finite() && seconds > 0.0 {
            self.elapsed = (self.elapsed + seconds) % DAY_LENGTH_SECONDS;
        }
    }

    pub fn phase(&self) -> f32 {
        self.elapsed / DAY_LENGTH_SECONDS
    }

    pub fn daylight(&self) -> f32 {
        let phase = self.elapsed / DAY_LENGTH_SECONDS * std::f32::consts::TAU;
        NIGHT_LIGHT + (1.0 - NIGHT_LIGHT) * (phase.cos() + 1.0) * 0.5
    }

    pub fn sky_color(&self) -> [f32; 3] {
        let daylight = self.daylight();
        [
            0.015 + 0.36 * daylight,
            0.02 + 0.48 * daylight,
            0.07 + 0.58 * daylight,
        ]
    }

    pub fn sun_direction(&self) -> [f32; 3] {
        let angle = self.elapsed / DAY_LENGTH_SECONDS * std::f32::consts::TAU;
        Vec3::new(angle.sin(), angle.cos(), 0.25)
            .normalize()
            .to_array()
    }
}

/// Lumière solaire propagée dans les cellules d'air des chunks chargés.
#[derive(Clone, Debug, Default)]
pub struct VoxelLighting {
    levels: HashMap<IVec3, u8>,
}

impl VoxelLighting {
    /// Reconstruit l'éclairage par propagation à six voisins. Le haut de chaque colonne ouverte
    /// est une source solaire ; la lumière perd un niveau à chaque cellule traversée.
    pub fn calculate(world: &World) -> Self {
        let mut light = Self::default();
        let mut queue = VecDeque::new();
        let mut coords = world.chunk_coords();
        coords.sort_by_key(|coord| (coord.x, coord.y, coord.z));

        let Some(min_y) = coords.iter().map(|coord| coord.y).min() else {
            return light;
        };
        let max_y = coords.iter().map(|coord| coord.y).max().unwrap_or(min_y);
        let columns: HashSet<_> = coords
            .iter()
            .flat_map(|coord| {
                let origin = *coord * CHUNK_SIZE as i32;
                (0..CHUNK_SIZE as i32).flat_map(move |x| {
                    (0..CHUNK_SIZE as i32).map(move |z| (origin.x + x, origin.z + z))
                })
            })
            .collect();
        for (x, z) in columns {
            let mut blocked = false;
            for y in (min_y * CHUNK_SIZE as i32..(max_y + 1) * CHUNK_SIZE as i32).rev() {
                let pos = IVec3::new(x, y, z);
                if !world.is_loaded_block(pos) {
                    continue;
                }
                if world.block_at(pos).is_solid() {
                    blocked = true;
                } else if !blocked {
                    light.levels.insert(pos, MAX_LIGHT);
                    queue.push_back(pos);
                }
            }
        }

        while let Some(pos) = queue.pop_front() {
            let level = light.levels[&pos];
            if level <= 1 {
                continue;
            }
            for offset in NEIGHBORS {
                let next = pos + offset;
                if !world.is_loaded_block(next) || world.block_at(next) != BlockType::Air {
                    continue;
                }
                let next_level = level - 1;
                if light.levels.get(&next).copied().unwrap_or(0) >= next_level {
                    continue;
                }
                light.levels.insert(next, next_level);
                queue.push_back(next);
            }
        }

        light
    }

    /// Chunks contenant des faces dont l'éclairage diffère de `previous`.
    pub fn changed_mesh_chunks(&self, world: &World, previous: &Self) -> HashSet<IVec3> {
        let mut changed_cells = HashSet::new();
        for (&pos, &level) in &self.levels {
            if previous.level_at(pos) != level {
                changed_cells.insert(pos);
            }
        }
        for (&pos, &level) in &previous.levels {
            if self.level_at(pos) != level {
                changed_cells.insert(pos);
            }
        }

        changed_cells
            .into_iter()
            .flat_map(|air| {
                NEIGHBORS.into_iter().filter_map(move |normal| {
                    let block = air - normal;
                    world
                        .block_at(block)
                        .is_solid()
                        .then(|| crate::voxel::world::split_block_pos(block).0)
                })
            })
            .collect()
    }

    /// Niveau de lumière d'une cellule ; les cellules non éclairées valent zéro.
    pub fn level_at(&self, pos: IVec3) -> u8 {
        self.levels.get(&pos).copied().unwrap_or(0)
    }

    /// Facteur d'éclairage minimal pour garder visibles les faces en ombre.
    pub fn factor_at(&self, pos: IVec3) -> f32 {
        0.2 + 0.8 * (self.level_at(pos) as f32 / MAX_LIGHT as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voxel::chunk::Chunk;

    #[test]
    fn le_cycle_jour_nuit_est_periodique_et_conserve_un_eclairage_minimal() {
        let mut cycle = DayNightCycle::default();
        assert!((cycle.daylight() - 1.0).abs() < 1e-6);

        cycle.advance(DAY_LENGTH_SECONDS / 2.0);
        assert!((cycle.daylight() - NIGHT_LIGHT).abs() < 1e-6);
        assert!(cycle.sky_color().iter().all(|channel| *channel > 0.0));

        cycle.advance(DAY_LENGTH_SECONDS / 2.0);
        assert!((cycle.daylight() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn les_temps_invalides_ne_modifient_pas_le_cycle() {
        let mut cycle = DayNightCycle::default();
        cycle.advance(40.0);
        let before = cycle.daylight();
        cycle.advance(f32::NAN);
        cycle.advance(f32::INFINITY);
        cycle.advance(-1.0);

        assert_eq!(cycle.daylight(), before);
    }

    #[test]
    fn la_phase_du_cycle_peut_etre_sauvegardee_et_restauree() {
        let mut cycle = DayNightCycle::default();
        cycle.advance(73.0);
        let restored = DayNightCycle::from_phase(cycle.phase()).unwrap();

        assert!((restored.daylight() - cycle.daylight()).abs() < 1e-6);
        assert!(DayNightCycle::from_phase(1.0).is_none());
        assert!(DayNightCycle::from_phase(f32::NAN).is_none());
    }

    #[test]
    fn la_direction_solaire_evolue_avec_le_cycle_et_reste_normalisee() {
        let mut cycle = DayNightCycle::default();
        let noon = Vec3::from_array(cycle.sun_direction());
        cycle.advance(DAY_LENGTH_SECONDS / 4.0);
        let sunset = Vec3::from_array(cycle.sun_direction());
        cycle.advance(DAY_LENGTH_SECONDS / 4.0);
        let midnight = Vec3::from_array(cycle.sun_direction());

        assert!(noon.y > 0.9);
        assert!(sunset.x > 0.9);
        assert!(midnight.y < -0.9);
        assert!((noon.length() - 1.0).abs() < 1e-6);
        assert!((sunset.length() - 1.0).abs() < 1e-6);
        assert!((midnight.length() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn la_lumiere_se_propage_dans_l_air_et_s_arrete_aux_blocs() {
        let mut chunk = Chunk::filled(BlockType::Air);
        chunk.set(IVec3::new(8, 8, 8), BlockType::Stone);
        let mut world = World::new();
        world.insert_chunk(IVec3::ZERO, chunk);

        let lighting = VoxelLighting::calculate(&world);

        assert_eq!(lighting.level_at(IVec3::new(8, 9, 8)), MAX_LIGHT);
        assert_eq!(lighting.level_at(IVec3::new(9, 8, 8)), MAX_LIGHT - 1);
        assert_eq!(lighting.level_at(IVec3::new(8, 8, 8)), 0);
    }

    #[test]
    fn les_chunks_vides_recoivent_le_soleil_mais_pas_les_blocs() {
        let mut chunk = Chunk::filled(BlockType::Air);
        chunk.set(IVec3::new(4, 15, 4), BlockType::Stone);
        let mut world = World::new();
        world.insert_chunk(IVec3::ZERO, chunk);

        let lighting = VoxelLighting::calculate(&world);

        assert_eq!(lighting.level_at(IVec3::new(4, 14, 4)), MAX_LIGHT);
        assert_eq!(lighting.level_at(IVec3::new(4, 15, 4)), 0);
        assert!(lighting.factor_at(IVec3::new(4, 14, 4)) > 0.99);
    }

    #[test]
    fn un_plafond_dans_le_chunk_superieur_bloque_la_lumiere_solaire() {
        let lower = Chunk::filled(BlockType::Air);
        let mut upper = Chunk::filled(BlockType::Air);
        upper.set(IVec3::new(4, 0, 4), BlockType::Stone);
        let mut world = World::new();
        world.insert_chunk(IVec3::ZERO, lower);
        world.insert_chunk(IVec3::Y, upper);

        let lighting = VoxelLighting::calculate(&world);

        assert_eq!(lighting.level_at(IVec3::new(4, 16, 4)), 0);
    }

    #[test]
    fn les_chunks_dont_les_faces_changent_de_lumiere_sont_signales() {
        let mut chunk = Chunk::filled(BlockType::Air);
        chunk.set(IVec3::new(8, 8, 8), BlockType::Stone);
        let mut world = World::new();
        world.insert_chunk(IVec3::ZERO, chunk);
        let before = VoxelLighting::calculate(&world);

        assert!(before.changed_mesh_chunks(&world, &before).is_empty());

        world.set_block(IVec3::new(8, 15, 8), BlockType::Stone);
        let after = VoxelLighting::calculate(&world);
        let changed = after.changed_mesh_chunks(&world, &before);

        assert!(changed.contains(&IVec3::ZERO));
        assert_ne!(
            after.factor_at(IVec3::new(8, 9, 8)),
            before.factor_at(IVec3::new(8, 9, 8))
        );
    }
}
