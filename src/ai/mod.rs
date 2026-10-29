use std::collections::{HashMap, VecDeque};

use glam::{IVec3, Vec3};

use crate::physics::collision::{move_aabb, Aabb};
use crate::voxel::block::BlockType;
use crate::voxel::world::World;

const DETECTION_RANGE: f32 = 18.0;
const ATTACK_RANGE: f32 = 1.4;
const MOVE_SPEED: f32 = 2.2;
const GRAVITY: f32 = 28.0;
const MAX_FALL_SPEED: f32 = 40.0;
const ATTACK_INTERVAL: f32 = 1.0;
const MOB_HALF_WIDTH: f32 = 0.3;
const MOB_HEIGHT: f32 = 1.4;
const MAX_ATTACK_HEIGHT_DIFFERENCE: f32 = 1.8;
const CONTACT_EPSILON: f32 = 1e-5;
const PATH_REFRESH_INTERVAL: f32 = 0.5;
const MAX_PATH_NODES: usize = 4096;
const STEP_HEIGHT: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MobState {
    Idle,
    Chasing,
    Attacking,
    Dead,
}

/// Entité hostile simple avec états idle, poursuite, attaque et mort.
#[derive(Clone, Debug)]
pub struct Mob {
    pub id: u32,
    pub position: Vec3,
    pub state: MobState,
    health: u8,
    attack_timer: f32,
    vertical_speed: f32,
    path: VecDeque<IVec3>,
    path_target: Option<IVec3>,
    path_timer: f32,
}

impl Mob {
    pub fn new(id: u32, position: Vec3) -> Self {
        Self {
            id,
            position,
            state: MobState::Idle,
            health: 3,
            attack_timer: 0.0,
            vertical_speed: 0.0,
            path: VecDeque::new(),
            path_target: None,
            path_timer: 0.0,
        }
    }

    /// Simule le mob et retourne les dégâts infligés au joueur pendant ce pas.
    pub fn update(&mut self, world: &World, player: Vec3, dt: f32) -> u8 {
        if self.state == MobState::Dead {
            return 0;
        }
        self.attack_timer = (self.attack_timer - dt).max(0.0);
        let toward_player = Vec3::new(player.x - self.position.x, 0.0, player.z - self.position.z);
        let distance = toward_player.length();
        let can_attack = distance <= ATTACK_RANGE
            && (player.y - (self.position.y + MOB_HEIGHT * 0.5)).abs()
                <= MAX_ATTACK_HEIGHT_DIFFERENCE;
        if distance > DETECTION_RANGE {
            self.state = MobState::Idle;
        } else if can_attack {
            self.state = MobState::Attacking;
        } else {
            self.state = MobState::Chasing;
        }

        let target_cell = player_feet_cell(player);
        self.path_timer = (self.path_timer - dt).max(0.0);
        if self.state == MobState::Chasing
            && (self.path_target != Some(target_cell) || self.path_timer == 0.0)
        {
            self.path = find_path(world, feet_cell(self.position), target_cell)
                .unwrap_or_default()
                .into();
            self.path_target = Some(target_cell);
            self.path_timer = PATH_REFRESH_INTERVAL;
        }
        let horizontal = if self.state == MobState::Chasing {
            self.path_direction() * MOVE_SPEED * dt
        } else {
            if self.state != MobState::Chasing {
                self.path.clear();
                self.path_target = None;
            }
            Vec3::ZERO
        };
        let requested_vertical = self.vertical_speed * dt;
        let delta = horizontal + Vec3::Y * requested_vertical;
        let bounds = Aabb {
            min: self.position - Vec3::new(MOB_HALF_WIDTH, 0.0, MOB_HALF_WIDTH),
            max: self.position + Vec3::new(MOB_HALF_WIDTH, MOB_HEIGHT, MOB_HALF_WIDTH),
        };
        let mut moved = move_aabb(world, bounds, delta);
        let horizontal_blocked =
            Vec3::new(moved.x, 0.0, moved.z).length_squared() + 1e-6 < horizontal.length_squared();
        let grounded = self.vertical_speed <= 0.0
            && (self.position.y - self.position.y.round()).abs() <= 0.05
            && world
                .block_at(feet_cell(self.position) - IVec3::Y)
                .is_solid();
        let mut stepped_up = false;
        if self.state == MobState::Chasing && grounded && horizontal_blocked {
            if let Some(step) = try_step_up(world, bounds, horizontal) {
                moved = step;
                stepped_up = true;
            }
        }
        self.position += moved;
        if stepped_up || (moved.y - requested_vertical).abs() > CONTACT_EPSILON {
            self.vertical_speed = 0.0;
        } else {
            self.vertical_speed = (self.vertical_speed - GRAVITY * dt).max(-MAX_FALL_SPEED);
        }

        if self.state == MobState::Attacking && self.attack_timer == 0.0 {
            self.attack_timer = ATTACK_INTERVAL;
            1
        } else {
            0
        }
    }

    /// Inflige un point de dégât et retourne vrai si le mob vient d'être vaincu.
    pub fn take_hit(&mut self) -> bool {
        self.take_damage(1)
    }

    /// Inflige plusieurs points de dégâts et retourne vrai si le mob vient d'être vaincu.
    pub fn take_damage(&mut self, damage: u8) -> bool {
        if self.state == MobState::Dead {
            return false;
        }
        self.health = self.health.saturating_sub(damage);
        if self.health == 0 {
            self.state = MobState::Dead;
            true
        } else {
            false
        }
    }

    pub fn mark_defeated(&mut self) {
        self.health = 0;
        self.state = MobState::Dead;
    }

    #[cfg(test)]
    pub fn health(&self) -> u8 {
        self.health
    }

    fn path_direction(&mut self) -> Vec3 {
        while let Some(&next) = self.path.front() {
            let target = Vec3::new(next.x as f32 + 0.5, next.y as f32, next.z as f32 + 0.5);
            let horizontal = Vec3::new(target.x - self.position.x, 0.0, target.z - self.position.z);
            if horizontal.length_squared() <= 0.01 && (target.y - self.position.y).abs() <= 0.1 {
                self.path.pop_front();
                continue;
            }
            return horizontal.normalize_or_zero();
        }
        Vec3::ZERO
    }
}

fn try_step_up(world: &World, bounds: Aabb, horizontal: Vec3) -> Option<Vec3> {
    let upward = move_aabb(world, bounds, Vec3::Y * STEP_HEIGHT);
    if (upward.y - STEP_HEIGHT).abs() > CONTACT_EPSILON {
        return None;
    }

    let raised_bounds = bounds.translated(upward);
    let stepped = move_aabb(world, raised_bounds, horizontal);
    let stepped_horizontal = Vec3::new(stepped.x, 0.0, stepped.z);
    if stepped_horizontal.length_squared() <= 1e-6 {
        return None;
    }

    let landing_bounds = raised_bounds.translated(stepped);
    let downward = move_aabb(world, landing_bounds, Vec3::NEG_Y * STEP_HEIGHT);
    Some(upward + stepped + downward)
}

fn feet_cell(position: Vec3) -> IVec3 {
    IVec3::new(
        position.x.floor() as i32,
        position.y.floor() as i32,
        position.z.floor() as i32,
    )
}

fn player_feet_cell(position: Vec3) -> IVec3 {
    feet_cell(position - Vec3::Y * 1.6)
}

fn find_path(world: &World, start: IVec3, target: IVec3) -> Option<Vec<IVec3>> {
    if !has_floor_and_clearance(world, start) {
        return None;
    }
    let mut frontier = VecDeque::from([start]);
    let mut previous = HashMap::from([(start, start)]);
    let mut destination = None;
    const HORIZONTAL: [IVec3; 4] = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z];
    const HEIGHT_ADJUSTMENTS: [i32; 4] = [1, 0, -1, -2];

    while let Some(current) = frontier.pop_front() {
        if (current.x - target.x).abs() + (current.z - target.z).abs() <= 1
            && (current.y - target.y).abs() <= 1
        {
            destination = Some(current);
            break;
        }
        if previous.len() >= MAX_PATH_NODES {
            break;
        }
        for horizontal in HORIZONTAL {
            for height in HEIGHT_ADJUSTMENTS {
                let next = current + horizontal + IVec3::Y * height;
                if previous.contains_key(&next) || !has_floor_and_clearance(world, next) {
                    continue;
                }
                previous.insert(next, current);
                frontier.push_back(next);
            }
        }
    }

    let mut cursor = destination?;
    let mut path = Vec::new();
    while cursor != start {
        path.push(cursor);
        cursor = previous[&cursor];
    }
    path.reverse();
    Some(path)
}

fn has_floor_and_clearance(world: &World, feet: IVec3) -> bool {
    world.block_at(feet - IVec3::Y).is_solid()
        && world.block_at(feet) == BlockType::Air
        && world.block_at(feet + IVec3::Y) == BlockType::Air
}

/// Position de départ déterministe au-dessus de la surface du terrain.
pub fn spawn_position(seed: u64, x: i32, z: i32) -> Vec3 {
    Vec3::new(
        x as f32 + 0.5,
        (surface_height(seed, x, z) + 1) as f32,
        z as f32 + 0.5,
    )
}

fn surface_height(seed: u64, x: i32, z: i32) -> i32 {
    crate::generation::surface_height(seed, x, z)
}

/// Retourne l'ennemi le plus proche devant le joueur et dans la portée de mêlée.
pub fn target_in_melee(mobs: &[Mob], origin: Vec3, direction: Vec3, range: f32) -> Option<&Mob> {
    let forward = Vec3::new(direction.x, 0.0, direction.z).normalize_or_zero();
    mobs.iter()
        .filter(|mob| mob.state != MobState::Dead)
        .filter_map(|mob| {
            let offset = Vec3::new(mob.position.x - origin.x, 0.0, mob.position.z - origin.z);
            let distance = offset.length();
            let facing = offset.normalize_or_zero().dot(forward);
            let vertical_distance = (mob.position.y + MOB_HEIGHT * 0.5 - origin.y).abs();
            (distance <= range
                && facing >= 0.65
                && vertical_distance <= MAX_ATTACK_HEIGHT_DIFFERENCE)
                .then_some((mob, distance))
        })
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(mob, _)| mob)
}

/// Retourne le mob le plus proche traversé par un rayon, avec la distance d'impact.
pub fn target_in_ray(
    mobs: &[Mob],
    origin: Vec3,
    direction: Vec3,
    range: f32,
) -> Option<(&Mob, f32)> {
    let direction = direction.normalize_or_zero();
    if direction == Vec3::ZERO || !range.is_finite() || range < 0.0 {
        return None;
    }
    mobs.iter()
        .filter(|mob| mob.state != MobState::Dead)
        .filter_map(|mob| {
            let bounds = Aabb {
                min: mob.position - Vec3::new(MOB_HALF_WIDTH, 0.0, MOB_HALF_WIDTH),
                max: mob.position + Vec3::new(MOB_HALF_WIDTH, MOB_HEIGHT, MOB_HALF_WIDTH),
            };
            ray_aabb_distance(origin, direction, bounds)
                .filter(|distance| *distance <= range)
                .map(|distance| (mob, distance))
        })
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
}

fn ray_aabb_distance(origin: Vec3, direction: Vec3, bounds: Aabb) -> Option<f32> {
    let mut near = 0.0_f32;
    let mut far = f32::INFINITY;
    for axis in 0..3 {
        if direction[axis].abs() <= f32::EPSILON {
            if origin[axis] < bounds.min[axis] || origin[axis] > bounds.max[axis] {
                return None;
            }
            continue;
        }
        let inverse = 1.0 / direction[axis];
        let first = (bounds.min[axis] - origin[axis]) * inverse;
        let second = (bounds.max[axis] - origin[axis]) * inverse;
        near = near.max(first.min(second));
        far = far.min(first.max(second));
        if near > far {
            return None;
        }
    }
    (far >= 0.0).then_some(near)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_mob_alterne_repos_poursuite_attaque_et_mort() {
        let mut mob = Mob::new(1, Vec3::new(4.0, 1.0, 0.0));
        let world = flat_world();
        let player = Vec3::new(0.5, 2.6, 8.5);

        mob.position.z = 8.5;
        assert_eq!(mob.update(&world, player, 0.1), 0);
        assert_eq!(mob.state, MobState::Chasing);
        assert!(mob.position.x < 4.0);

        mob.position = Vec3::new(1.0, 1.0, 0.0);
        assert_eq!(mob.update(&world, Vec3::ZERO, 0.1), 1);
        assert_eq!(mob.state, MobState::Attacking);
        assert_eq!(mob.update(&world, Vec3::ZERO, 0.1), 0);

        assert!(!mob.take_hit());
        assert!(!mob.take_hit());
        assert!(mob.take_hit());
        assert_eq!(mob.state, MobState::Dead);
        assert_eq!(mob.update(&world, Vec3::ZERO, 1.0), 0);
    }

    #[test]
    fn le_ray_touche_le_mob_le_plus_proche_devant_lui() {
        let mobs = [
            Mob::new(1, Vec3::new(0.0, 0.0, -5.0)),
            Mob::new(2, Vec3::new(0.0, 0.0, -3.0)),
        ];

        let (target, distance) = target_in_ray(&mobs, Vec3::ZERO, Vec3::NEG_Z, 10.0).unwrap();
        assert_eq!(target.id, 2);
        assert!((distance - 2.7).abs() < 1e-5);
        assert!(target_in_ray(&mobs, Vec3::ZERO, Vec3::NEG_Z, 2.0).is_none());
        assert!(target_in_ray(&mobs, Vec3::ZERO, Vec3::X, 10.0).is_none());
    }

    #[test]
    fn les_degats_multiples_peuvent_vaincre_un_mob_en_un_coup() {
        let mut mob = Mob::new(1, Vec3::ZERO);
        assert!(mob.take_damage(3));
        assert_eq!(mob.state, MobState::Dead);
        assert!(!mob.take_damage(5));
    }

    fn flat_world() -> World {
        use crate::voxel::chunk::{Chunk, CHUNK_SIZE};

        let mut chunk = Chunk::filled(BlockType::Air);
        for x in 0..CHUNK_SIZE as i32 {
            for z in 0..CHUNK_SIZE as i32 {
                chunk.set(IVec3::new(x, 0, z), BlockType::Stone);
            }
        }
        let mut world = World::new();
        world.insert_chunk(IVec3::ZERO, chunk);
        world
    }

    #[test]
    fn un_mob_sans_chemin_ne_charge_pas_directement_dans_un_mur() {
        use crate::voxel::chunk::{Chunk, CHUNK_SIZE};

        let mut chunk = Chunk::filled(BlockType::Air);
        for x in 0..CHUNK_SIZE as i32 {
            for z in 0..CHUNK_SIZE as i32 {
                chunk.set(IVec3::new(x, 0, z), BlockType::Stone);
            }
        }
        for y in 1..=2 {
            for z in 0..CHUNK_SIZE as i32 {
                chunk.set(IVec3::new(4, y, z), BlockType::Stone);
            }
        }
        let mut world = World::new();
        world.insert_chunk(IVec3::ZERO, chunk);
        let mut mob = Mob::new(1, Vec3::new(2.5, 1.0, 8.5));

        for _ in 0..60 {
            mob.update(&world, Vec3::new(6.5, 2.6, 8.5), 1.0 / 60.0);
        }

        assert!(mob.position.x < 3.7);
    }

    #[test]
    fn un_mob_loin_reste_inactif() {
        let mut mob = Mob::new(2, Vec3::new(100.0, 1.0, 0.0));

        assert_eq!(mob.update(&World::new(), Vec3::ZERO, 0.1), 0);
        assert_eq!(mob.state, MobState::Idle);
    }

    #[test]
    fn un_mob_tombe_et_s_arrete_sur_le_sol() {
        use crate::voxel::block::BlockType;
        use crate::voxel::chunk::{Chunk, CHUNK_SIZE};

        let mut chunk = Chunk::filled(BlockType::Air);
        for x in 0..CHUNK_SIZE as i32 {
            for z in 0..CHUNK_SIZE as i32 {
                chunk.set(glam::IVec3::new(x, 0, z), BlockType::Stone);
            }
        }
        let mut world = World::new();
        world.insert_chunk(glam::IVec3::ZERO, chunk);
        let mut mob = Mob::new(1, Vec3::new(8.5, 5.0, 8.5));

        for _ in 0..120 {
            mob.update(&world, Vec3::new(12.0, 2.7, 8.5), 1.0 / 60.0);
        }

        assert!((mob.position.y - 1.0).abs() < 1e-4);
        assert!(mob.position.x > 8.5);
    }

    #[test]
    fn les_attaques_ne_traversent_pas_une_difference_de_hauteur_importante() {
        let mut mob = Mob::new(1, Vec3::new(0.0, 8.0, 0.0));

        assert_eq!(mob.update(&World::new(), Vec3::new(0.0, 0.0, 0.5), 0.1), 0);
        assert_eq!(mob.state, MobState::Chasing);
    }

    #[test]
    fn le_pathfinding_contourne_un_mur_sur_une_surface_marchable() {
        use crate::voxel::chunk::{Chunk, CHUNK_SIZE};

        let mut chunk = Chunk::filled(crate::voxel::block::BlockType::Air);
        for x in 0..CHUNK_SIZE as i32 {
            for z in 0..CHUNK_SIZE as i32 {
                chunk.set(IVec3::new(x, 0, z), BlockType::Stone);
            }
        }
        for y in 1..=2 {
            for z in 2..=4 {
                chunk.set(IVec3::new(4, y, z), BlockType::Stone);
            }
        }
        let mut world = World::new();
        world.insert_chunk(IVec3::ZERO, chunk);

        let path = find_path(&world, IVec3::new(2, 1, 3), IVec3::new(6, 1, 3)).unwrap();

        assert!(path.len() > 4);
        assert!(!path.contains(&IVec3::new(4, 1, 3)));
        assert_eq!(path.last(), Some(&IVec3::new(5, 1, 3)));
    }

    #[test]
    fn le_pathfinding_emprunte_une_marche_ascendante() {
        use crate::voxel::chunk::{Chunk, CHUNK_SIZE};

        let mut chunk = Chunk::filled(BlockType::Air);
        for x in 0..CHUNK_SIZE as i32 {
            for z in 0..CHUNK_SIZE as i32 {
                chunk.set(IVec3::new(x, 0, z), BlockType::Stone);
            }
        }
        for x in 4..=8 {
            chunk.set(IVec3::new(x, 1, 3), BlockType::Stone);
        }
        let mut world = World::new();
        world.insert_chunk(IVec3::ZERO, chunk);

        let path = find_path(&world, IVec3::new(2, 1, 3), IVec3::new(7, 2, 3)).unwrap();

        assert!(path.contains(&IVec3::new(4, 2, 3)));
        assert_eq!(path.last(), Some(&IVec3::new(6, 2, 3)));
    }

    #[test]
    fn le_pathfinding_traverse_une_frontiere_entre_chunks_charges() {
        use crate::voxel::chunk::{Chunk, CHUNK_SIZE};

        let mut world = World::new();
        for chunk_x in 0..=1 {
            let mut chunk = Chunk::filled(BlockType::Air);
            for x in 0..CHUNK_SIZE as i32 {
                for z in 0..CHUNK_SIZE as i32 {
                    chunk.set(IVec3::new(x, 0, z), BlockType::Stone);
                }
            }
            world.insert_chunk(IVec3::new(chunk_x, 0, 0), chunk);
        }

        let path = find_path(&world, IVec3::new(14, 1, 3), IVec3::new(18, 1, 3)).unwrap();

        assert!(path.contains(&IVec3::new(16, 1, 3)));
        assert!(path.last().unwrap().x >= 17);
    }

    #[test]
    fn un_mob_grimpe_une_marche_pour_rejoindre_le_joueur() {
        use crate::voxel::chunk::{Chunk, CHUNK_SIZE};

        let mut chunk = Chunk::filled(BlockType::Air);
        for x in 0..CHUNK_SIZE as i32 {
            for z in 0..CHUNK_SIZE as i32 {
                chunk.set(IVec3::new(x, 0, z), BlockType::Stone);
            }
        }
        for x in 4..=8 {
            chunk.set(IVec3::new(x, 1, 3), BlockType::Stone);
        }
        let mut world = World::new();
        world.insert_chunk(IVec3::ZERO, chunk);
        let mut mob = Mob::new(1, Vec3::new(2.5, 1.0, 3.5));

        for _ in 0..240 {
            mob.update(&world, Vec3::new(7.5, 3.6, 3.5), 1.0 / 60.0);
        }

        assert!(
            mob.position.x > 5.0,
            "le mob n'a pas franchi la marche : {:?}",
            mob.position
        );
        assert!((mob.position.y - 2.0).abs() < 1e-4);
    }

    #[test]
    fn le_combat_cible_une_entite_devant_et_dans_la_portee() {
        let mobs = [
            Mob::new(1, Vec3::new(0.0, 1.0, 3.0)),
            Mob::new(2, Vec3::new(0.0, 1.0, -1.0)),
        ];

        assert_eq!(
            target_in_melee(&mobs, Vec3::ZERO, -Vec3::Z, 2.0).map(|mob| mob.id),
            Some(2)
        );
    }

    #[test]
    fn le_combat_ne_cible_pas_un_ennemi_trop_haut_ou_trop_bas() {
        let mobs = [Mob::new(1, Vec3::new(0.0, 8.0, -1.0))];

        assert!(target_in_melee(&mobs, Vec3::ZERO, -Vec3::Z, 2.0).is_none());
    }

    #[test]
    fn un_mob_ne_peut_pas_etre_touche_deux_fois_apres_la_mort() {
        let mut mob = Mob::new(1, Vec3::ZERO);
        mob.take_hit();
        mob.take_hit();
        assert!(mob.take_hit());

        assert!(!mob.take_hit());
        assert_eq!(mob.state, MobState::Dead);
    }
}
