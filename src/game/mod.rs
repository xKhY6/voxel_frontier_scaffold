use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use glam::{IVec3, Vec3};
use serde::{Deserialize, Serialize};
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

use crate::ai::{self, Mob, MobState};
use crate::audio::{AudioPlayer, SoundEffect};
use crate::camera::Camera;
use crate::crafting::{CraftError, Inventory};
use crate::generation::{generate_area, generate_chunk, surface_height};
use crate::input::InputState;
use crate::lighting::VoxelLighting;
use crate::physics::collision::{move_aabb, Aabb};
use crate::render::{FrameState, HudState, OpenGlWindow, Renderer, SceneLighting};
use crate::save;
use crate::voxel::block::BlockType;
use crate::voxel::raycast::raycast;
use crate::voxel::world::{chunks_touching, split_block_pos, World};
use crate::weapons::WeaponInventory;

/// Pas de temps fixe de la simulation (environ 60 Hz).
const FIXED_STEP: Duration = Duration::from_micros(16_667);

/// Temps réel maximal rattrapé en une seule image, pour éviter une spirale de mises à jour après une pause.
const MAX_FRAME_TIME: Duration = Duration::from_millis(250);

/// Argument de ligne de commande : affiche le message de démarrage et quitte sans ouvrir de fenêtre.
const CHECK_FLAG: &str = "--check";
const SETTINGS_PATH: &str = "voxel_frontier_settings.json";

/// Radians de rotation par unité de mouvement brut de souris.
const BASE_MOUSE_SENSITIVITY: f32 = 0.002;
const DEFAULT_MOUSE_SENSITIVITY_LEVEL: u8 = 3;
const DEFAULT_RENDER_DISTANCE: i32 = WORLD_LOAD_RADIUS;
const MIN_RENDER_DISTANCE: i32 = 1;
const MAX_RENDER_DISTANCE: i32 = 3;

/// Graine du monde : une même graine redonne le même terrain.
const WORLD_SEED: u64 = 2026;

/// Rayon de génération autour du joueur, en chunks.
const WORLD_LOAD_RADIUS: i32 = 2;

/// Couches verticales couvrant toutes les hauteurs du terrain et la végétation.
const WORLD_CHUNK_LAYERS: i32 = 3;

/// Vitesse de marche, en blocs par seconde.
const MOVE_SPEED: f32 = 5.0;

/// Accélération de la gravité, en blocs par seconde carrée.
const GRAVITY: f32 = 28.0;

/// Vitesse initiale du saut, en blocs par seconde : apogée d'environ 1,2 bloc.
const JUMP_SPEED: f32 = 8.5;

/// Vitesse de chute maximale, en blocs par seconde.
const MAX_FALL_SPEED: f32 = 50.0;

/// Tolérance sur un déplacement vertical pour détecter un obstacle.
const CONTACT_EPSILON: f32 = 1e-5;

/// Fichier de sauvegarde, relatif au dossier de lancement (F5 sauvegarde, F9 charge).
const SAVE_PATH: &str = "voxel_frontier_save.json";

/// Hauteur des yeux au-dessus des pieds, en blocs.
const EYE_HEIGHT: f32 = 1.6;

/// Taille de la boîte du joueur : hauteur totale et demi-largeur, en blocs.
const PLAYER_HEIGHT: f32 = 1.8;
const PLAYER_HALF_WIDTH: f32 = 0.3;

/// Portée d'édition, en blocs.
const REACH: f32 = 6.0;

/// En dessous de cette hauteur, le joueur est replacé au point de départ.
const FALL_RESET_HEIGHT: f32 = -32.0;
const PLAYER_DAMAGE_COOLDOWN: f32 = 0.65;
const PLAYER_ATTACK_INTERVAL: f32 = 0.3;

#[derive(Debug, Deserialize, Serialize)]
struct UserSettings {
    #[serde(default = "default_mouse_sensitivity_level")]
    mouse_sensitivity_level: u8,
    #[serde(default = "default_render_distance")]
    render_distance: i32,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            mouse_sensitivity_level: DEFAULT_MOUSE_SENSITIVITY_LEVEL,
            render_distance: DEFAULT_RENDER_DISTANCE,
        }
    }
}

fn default_mouse_sensitivity_level() -> u8 {
    DEFAULT_MOUSE_SENSITIVITY_LEVEL
}

fn default_render_distance() -> i32 {
    DEFAULT_RENDER_DISTANCE
}

fn load_user_settings(path: &Path) -> Result<UserSettings, String> {
    let contents = match fs::read(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(UserSettings::default());
        }
        Err(error) => return Err(format!("lecture des options impossible : {error}")),
    };
    let settings: UserSettings = serde_json::from_slice(&contents)
        .map_err(|error| format!("fichier d'options invalide : {error}"))?;
    if !(1..=5).contains(&settings.mouse_sensitivity_level) {
        return Err("la sensibilité souris doit être comprise entre 1 et 5".to_owned());
    }
    if !(MIN_RENDER_DISTANCE..=MAX_RENDER_DISTANCE).contains(&settings.render_distance) {
        return Err(format!(
            "la distance d'affichage doit être comprise entre {MIN_RENDER_DISTANCE} et {MAX_RENDER_DISTANCE}"
        ));
    }
    Ok(settings)
}

fn save_user_settings(path: &Path, settings: &UserSettings) -> Result<(), String> {
    let contents = serde_json::to_vec_pretty(settings)
        .map_err(|error| format!("sérialisation des options impossible : {error}"))?;
    fs::write(path, contents).map_err(|error| format!("écriture des options impossible : {error}"))
}

/// Transforme le temps réel écoulé en nombre de pas fixes à simuler.
#[derive(Debug, Default)]
struct FixedStepClock {
    accumulator: Duration,
}

impl FixedStepClock {
    /// Ajoute le temps écoulé (borné par `MAX_FRAME_TIME`) et retourne le nombre de pas à exécuter.
    /// Le reliquat inférieur à `FIXED_STEP` est conservé pour la prochaine image.
    fn advance(&mut self, elapsed: Duration) -> u32 {
        self.accumulator += elapsed.min(MAX_FRAME_TIME);

        let mut steps = 0;
        while self.accumulator >= FIXED_STEP {
            self.accumulator -= FIXED_STEP;
            steps += 1;
        }
        steps
    }
}

/// Vitesse verticale du joueur (blocs/s, positive vers le haut) et contact avec le sol.
#[derive(Debug, Default, Clone, Copy)]
struct VerticalState {
    speed: f32,
    on_ground: bool,
}

impl VerticalState {
    /// Déplacement vertical du pas `dt` : saut si au sol et demandé, puis gravité bornée.
    fn next_delta(&mut self, jump: bool, dt: f32) -> f32 {
        if jump && self.on_ground {
            self.speed = JUMP_SPEED;
        }
        self.speed = (self.speed - GRAVITY * dt).max(-MAX_FALL_SPEED);
        self.speed * dt
    }

    /// Met à jour le contact : un déplacement vertical amputé signale un obstacle.
    fn resolve(&mut self, requested: f32, moved: f32) {
        let blocked = (moved - requested).abs() > CONTACT_EPSILON;
        self.on_ground = blocked && requested < 0.0;
        if blocked {
            self.speed = 0.0;
        }
    }
}

/// Un pas du joueur dont les yeux sont en `eye` : déplacement horizontal voulu, puis gravité ou saut.
/// Retourne le déplacement permis par les collisions.
fn step_body(
    world: &World,
    eye: Vec3,
    horizontal: Vec3,
    vertical: &mut VerticalState,
    jump: bool,
    dt: f32,
) -> Vec3 {
    let vertical_delta = vertical.next_delta(jump, dt);
    let requested = horizontal + Vec3::Y * vertical_delta;
    let moved = move_aabb(world, player_aabb(eye), requested);
    vertical.resolve(vertical_delta, moved.y);
    moved
}

/// Boîte du joueur à partir de la position de ses yeux.
fn player_aabb(eye: Vec3) -> Aabb {
    Aabb {
        min: eye - Vec3::new(PLAYER_HALF_WIDTH, EYE_HEIGHT, PLAYER_HALF_WIDTH),
        max: eye
            + Vec3::new(
                PLAYER_HALF_WIDTH,
                PLAYER_HEIGHT - EYE_HEIGHT,
                PLAYER_HALF_WIDTH,
            ),
    }
}

/// État de l'application winit : monde, rendu, horloge et erreur éventuelle de démarrage.
struct App {
    gl_window: Option<OpenGlWindow>,
    renderer: Option<Renderer>,
    audio: Option<AudioPlayer>,
    audio_error: Option<String>,
    clock: FixedStepClock,
    last_frame: Option<Instant>,
    input: InputState,
    camera: Camera,
    vertical: VerticalState,
    world: World,
    last_stream_centers: Option<Vec<IVec3>>,
    chunk_cache_path: PathBuf,
    mobs: Vec<Mob>,
    player_health: u8,
    player_damage_cooldown: f32,
    player_attack_cooldown: f32,
    weapons: WeaponInventory,
    weapon_mode: bool,
    inventory: Inventory,
    lighting: VoxelLighting,
    day_night: crate::lighting::DayNightCycle,
    selected_block: BlockType,
    paused: bool,
    crafting_open: bool,
    options_open: bool,
    mouse_sensitivity_level: u8,
    render_distance: i32,
    victory: bool,
    main_menu: bool,
    cursor_captured: bool,
    status: String,
    error: Option<String>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            gl_window: None,
            renderer: None,
            audio: None,
            audio_error: None,
            clock: FixedStepClock::default(),
            last_frame: None,
            input: InputState::default(),
            camera: Camera::default(),
            vertical: VerticalState::default(),
            world: World::default(),
            last_stream_centers: None,
            chunk_cache_path: new_chunk_cache_path(),
            mobs: Vec::new(),
            player_health: 10,
            player_damage_cooldown: 0.0,
            player_attack_cooldown: 0.0,
            weapons: WeaponInventory::default(),
            weapon_mode: false,
            inventory: Inventory::default(),
            lighting: VoxelLighting::default(),
            day_night: crate::lighting::DayNightCycle::default(),
            selected_block: BlockType::Stone,
            paused: false,
            crafting_open: false,
            options_open: false,
            mouse_sensitivity_level: DEFAULT_MOUSE_SENSITIVITY_LEVEL,
            render_distance: DEFAULT_RENDER_DISTANCE,
            victory: false,
            main_menu: false,
            cursor_captured: false,
            status: String::new(),
            error: None,
        }
    }
}

impl App {
    /// Pas de simulation : applique la souris, puis le déplacement borné par les collisions.
    fn update(&mut self) {
        if self.paused || self.crafting_open || self.options_open || self.victory || self.main_menu
        {
            return;
        }
        let (dx, dy) = self.input.take_mouse_delta();
        // Souris vers la droite : yaw décroissant ; vers le bas : pitch décroissant.
        self.camera.rotate(
            -dx as f32 * mouse_sensitivity(self.mouse_sensitivity_level),
            -dy as f32 * mouse_sensitivity(self.mouse_sensitivity_level),
        );

        let dt = FIXED_STEP.as_secs_f32();
        self.day_night.advance(dt);
        self.player_attack_cooldown = (self.player_attack_cooldown - dt).max(0.0);
        self.weapons.advance(dt);
        if self.weapon_mode
            && self.input.is_mouse_button_down(MouseButton::Left)
            && self.weapons.is_ready()
        {
            self.fire_weapon();
        }
        let horizontal = self.horizontal_delta(dt);
        let jump = self.input.is_key_down(KeyCode::Space);
        let moved = step_body(
            &self.world,
            self.camera.position,
            horizontal,
            &mut self.vertical,
            jump,
            dt,
        );
        self.camera.position += moved;

        if fell_out_of_world(self.camera.position) {
            self.camera.position = spawn_position();
            self.vertical = VerticalState::default();
        }
        let damage: u8 = self
            .mobs
            .iter_mut()
            .map(|mob| mob.update(&self.world, self.camera.position, dt))
            .sum();
        (self.player_health, self.player_damage_cooldown) =
            resolve_player_damage(self.player_health, self.player_damage_cooldown, damage, dt);
        if damage > 0 {
            self.play_sound(SoundEffect::Hurt);
        }
        if self.player_health == 0 {
            self.camera.position = spawn_position();
            self.vertical = VerticalState::default();
            self.player_health = 10;
            self.player_damage_cooldown = 0.0;
            self.player_attack_cooldown = 0.0;
            self.status = "Vous avez été vaincu : retour au point de départ".to_owned();
            self.update_window_title();
        }
    }

    /// Déplacement horizontal voulu pendant `dt` secondes, dans le plan horizontal de la caméra.
    fn horizontal_delta(&self, dt: f32) -> Vec3 {
        let forward = self.camera.forward();
        let flat = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
        let right = flat.cross(Vec3::Y);

        let mut dir = Vec3::ZERO;
        if self.input.is_key_down(KeyCode::KeyW) {
            dir += flat;
        }
        if self.input.is_key_down(KeyCode::KeyS) {
            dir -= flat;
        }
        if self.input.is_key_down(KeyCode::KeyD) {
            dir += right;
        }
        if self.input.is_key_down(KeyCode::KeyA) {
            dir -= right;
        }
        // Normaliser évite d'aller plus vite en diagonale.
        dir.normalize_or_zero() * MOVE_SPEED * dt
    }

    /// Dessine une image avec la caméra courante. Sans effet si le rendu n'est pas prêt.
    fn render(&mut self, event_loop: &ActiveEventLoop) {
        let (Some(gl_window), Some(renderer)) = (&self.gl_window, &self.renderer) else {
            return;
        };
        // Fenêtre réduite : taille nulle, pas de matrice de projection valide.
        let size = gl_window.window().inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }

        let aspect = size.width as f32 / size.height as f32;
        let view_proj = self.camera.projection_matrix(aspect) * self.camera.view_matrix();
        let entities: Vec<_> = self
            .mobs
            .iter()
            .filter(|mob| mob.state != MobState::Dead)
            .map(|mob| (mob.position, Vec3::new(0.6, 1.4, 0.6)))
            .collect();
        let enemies_remaining = entities.len();
        renderer.draw(
            gl_window.gl(),
            size.width,
            size.height,
            view_proj,
            &entities,
            FrameState {
                hud: HudState {
                    player_health: self.player_health,
                    enemies_remaining,
                    selected_hotbar_slot: hotbar_slot_for_block(self.selected_block),
                    selected_weapon_slot: if self.weapon_mode {
                        self.weapons.selected_slot()
                    } else {
                        0
                    },
                    weapon_ammo: self.weapons.ammo().0,
                    weapon_reserve: self.weapons.ammo().1,
                    hotbar_counts: [
                        self.inventory.count(BlockType::Grass),
                        self.inventory.count(BlockType::Dirt),
                        self.inventory.count(BlockType::Stone),
                        self.inventory.count(BlockType::Brick),
                        self.inventory.count(BlockType::Sand),
                        self.inventory.count(BlockType::Snow),
                        self.inventory.count(BlockType::Wood),
                        self.inventory.count(BlockType::Leaves),
                        self.inventory.count(BlockType::Planks),
                    ],
                    paused: self.paused,
                    crafting_open: self.crafting_open,
                    can_craft_bricks: can_craft_bricks(&self.inventory),
                    can_craft_planks: can_craft_planks(&self.inventory),
                    stone_count: self.inventory.count(BlockType::Stone),
                    dirt_count: self.inventory.count(BlockType::Dirt),
                    wood_count: self.inventory.count(BlockType::Wood),
                    options_open: self.options_open,
                    mouse_sensitivity_level: self.mouse_sensitivity_level,
                    render_distance: self.render_distance,
                    victory: self.victory,
                    main_menu: self.main_menu,
                    can_continue: Path::new(SAVE_PATH).is_file(),
                },
                lighting: SceneLighting {
                    ambient: self.day_night.daylight(),
                    sky_color: self.day_night.sky_color(),
                    sun_direction: self.day_night.sun_direction(),
                },
            },
        );

        if let Err(err) = gl_window.swap_buffers() {
            self.fail(
                event_loop,
                format!("affichage de l'image impossible : {err}"),
            );
        }
    }

    /// Clic gauche : retire le bloc visé. Clic droit : pose le matériau choisi sur la face visée.
    fn edit_block(&mut self, button: MouseButton) -> Result<(), String> {
        let Some(hit) = raycast(
            &self.world,
            self.camera.position,
            self.camera.forward(),
            REACH,
        ) else {
            return Ok(());
        };
        let (cell, block) = match button {
            MouseButton::Left => {
                let mined = self.world.block_at(hit.block);
                if !mined.is_solid() {
                    return Ok(());
                }
                if !self.inventory.can_add(mined, 1) {
                    self.status = format!("Inventaire plein : impossible de récupérer {mined:?}");
                    self.update_window_title();
                    return Ok(());
                }
                (hit.block, BlockType::Air)
            }
            MouseButton::Right => {
                if self.inventory.count(self.selected_block) == 0 {
                    self.status = format!("Aucun bloc {:?} disponible", self.selected_block);
                    self.update_window_title();
                    return Ok(());
                }
                let cell = hit.block + hit.normal;
                // Interdit de poser un bloc dans la boîte du joueur.
                if player_aabb(self.camera.position).overlaps_cell(cell) {
                    return Ok(());
                }
                (cell, self.selected_block)
            }
            _ => return Ok(()),
        };
        let mined = self.world.block_at(cell);
        if !self.world.set_block(cell, block) {
            self.status = "Impossible de modifier un chunk non chargé".to_owned();
            self.update_window_title();
            return Ok(());
        }
        if block == BlockType::Air {
            if !self.inventory.add(mined, 1) {
                if !self.world.set_block(cell, mined) {
                    return Err("récolte impossible et restauration du bloc échouée".to_owned());
                }
                return Err("l'inventaire a changé pendant la récolte".to_owned());
            }
            self.status = format!("Récolté : {mined:?}");
            self.play_sound(SoundEffect::Mine);
        } else {
            if !self.inventory.remove(block, 1) {
                if !self.world.set_block(cell, mined) {
                    return Err("pose impossible et restauration du bloc échouée".to_owned());
                }
                return Err("le bloc sélectionné n'est plus disponible".to_owned());
            }
            self.status = format!("Bloc posé : {block:?}");
            self.play_sound(SoundEffect::Place);
        }
        self.update_window_title();
        self.remesh(&chunks_touching(cell))
    }

    /// Reconstruit le maillage GPU des chunks donnés à partir de l'état du monde.
    fn remesh(&mut self, coords: &[IVec3]) -> Result<(), String> {
        let Some(gl_window) = &self.gl_window else {
            return Ok(());
        };
        if self.renderer.is_none() {
            return Ok(());
        }
        let lighting = VoxelLighting::calculate(&self.world);
        let mut dirty: HashSet<_> = coords.iter().copied().collect();
        dirty.extend(lighting.changed_mesh_chunks(&self.world, &self.lighting));
        self.lighting = lighting;
        let Some(renderer) = self.renderer.as_mut() else {
            return Ok(());
        };
        let mut dirty: Vec<_> = dirty.into_iter().collect();
        dirty.sort_by_key(|coord| (coord.x, coord.y, coord.z));
        for coord in dirty {
            let mesh = crate::voxel::mesh::build_chunk_mesh_with_lighting(
                &self.world,
                coord,
                &self.lighting,
            );
            renderer.set_chunk_mesh(gl_window.gl(), coord, &mesh)?;
        }
        Ok(())
    }

    fn stream_chunks(&mut self) -> Result<(), String> {
        let player_center = split_block_pos(self.camera.position.floor().as_ivec3()).0;
        let mob_centers: Vec<_> = self
            .mobs
            .iter()
            .filter(|mob| mob.state != MobState::Dead)
            .map(|mob| split_block_pos(mob.position.floor().as_ivec3()).0)
            .collect();
        let mut centers = vec![player_center];
        centers.extend(mob_centers.iter().copied());
        if self.last_stream_centers.as_ref() == Some(&centers) {
            return Ok(());
        }
        self.last_stream_centers = Some(centers);

        let mut added = ensure_chunks_around_radius(
            &mut self.world,
            player_center,
            self.render_distance,
            &self.chunk_cache_path,
        )?;
        for &center in &mob_centers {
            added.extend(ensure_chunks_around_radius(
                &mut self.world,
                center,
                1,
                &self.chunk_cache_path,
            )?);
        }
        let mut retained_regions = vec![(player_center, unload_radius(self.render_distance))];
        retained_regions.extend(mob_centers.into_iter().map(|center| (center, 1)));
        let evicted = evict_chunks_outside_regions(
            &mut self.world,
            &retained_regions,
            &self.chunk_cache_path,
        )?;

        if added.is_empty() && evicted.is_empty() {
            return Ok(());
        }

        let mut dirty = added;
        dirty.extend(evicted);
        let affected = dirty.clone();
        for coord in affected {
            dirty.extend([
                coord + IVec3::X,
                coord - IVec3::X,
                coord + IVec3::Y,
                coord - IVec3::Y,
                coord + IVec3::Z,
                coord - IVec3::Z,
            ]);
        }
        self.remesh(&dirty)
    }

    /// Écrit le monde dans `SAVE_PATH`. Une erreur est affichée sans arrêter le jeu.
    fn save_game(&mut self) {
        let defeated: Vec<_> = self
            .mobs
            .iter()
            .filter(|mob| mob.state == MobState::Dead)
            .map(|mob| mob.id)
            .collect();
        match save::save_game_state_with_cache(
            Path::new(SAVE_PATH),
            &self.chunk_cache_path,
            &self.world,
            &self.inventory,
            &defeated,
            save::PlayerSaveState {
                position: self.camera.position.to_array(),
                health: self.player_health,
                yaw: self.camera.yaw,
                pitch: self.camera.pitch,
                day_phase: self.day_night.phase(),
                weapons: Some(self.weapons.save_state()),
            },
        ) {
            Ok(()) => {
                self.status = format!("Monde sauvegardé dans {SAVE_PATH}");
                println!("{}", self.status);
            }
            Err(err) => {
                self.status = format!("Sauvegarde impossible : {err}");
                eprintln!("{}", self.status);
            }
        }
        self.update_window_title();
    }

    /// Remplace le monde par celui de `SAVE_PATH`. En cas d'erreur, le monde courant est conservé.
    fn load_game(&mut self) -> bool {
        let (world, inventory, defeated, player) =
            match save::load_game_state_with_cache(Path::new(SAVE_PATH), &self.chunk_cache_path) {
                Ok(data) => data,
                Err(err) => {
                    self.status = format!("Chargement impossible : {err}");
                    eprintln!("{}", self.status);
                    self.update_window_title();
                    return false;
                }
            };
        // Les chunks absents du monde chargé doivent disparaître de l'affichage.
        let mut coords = self.world.chunk_coords();
        self.world = world;
        self.inventory = inventory;
        if let Some(state) = player {
            self.camera = Camera {
                position: Vec3::from_array(state.position),
                yaw: state.yaw,
                pitch: state.pitch,
            };
            self.player_health = state.health;
            self.player_damage_cooldown = 0.0;
            self.player_attack_cooldown = 0.0;
            self.weapons = state
                .weapons
                .and_then(WeaponInventory::from_save_state)
                .unwrap_or_default();
            self.day_night = crate::lighting::DayNightCycle::from_phase(state.day_phase)
                .expect("la phase de cycle a été validée au chargement");
        } else {
            self.camera = Camera {
                position: spawn_position(),
                pitch: -0.3,
                ..Camera::default()
            };
            self.player_health = 10;
            self.player_damage_cooldown = 0.0;
            self.player_attack_cooldown = 0.0;
            self.weapons = WeaponInventory::default();
            self.day_night = crate::lighting::DayNightCycle::default();
        }
        self.main_menu = false;
        self.crafting_open = false;
        self.options_open = false;
        self.vertical = VerticalState::default();
        self.last_stream_centers = None;
        self.mobs = vec![
            Mob::new(1, ai::spawn_position(WORLD_SEED, 8, 8)),
            Mob::new(2, ai::spawn_position(WORLD_SEED, -10, -8)),
        ];
        for mob in &mut self.mobs {
            if defeated.contains(&mob.id) {
                mob.mark_defeated();
            }
        }
        self.victory = self.mobs.iter().all(|mob| mob.state == MobState::Dead);
        coords.extend(self.world.chunk_coords());
        self.status = match self.remesh(&coords) {
            Ok(()) if self.victory => "Victoire déjà accomplie : R pour recommencer".to_owned(),
            Ok(()) => "Monde et inventaire chargés".to_owned(),
            Err(err) => {
                let message = format!("Monde chargé, mais affichage incomplet : {err}");
                eprintln!("{message}");
                message
            }
        };
        self.update_window_title();
        true
    }

    /// Affiche les commandes essentielles et le matériau actif dans la barre de titre.
    fn update_window_title(&self) {
        let Some(gl_window) = &self.gl_window else {
            return;
        };
        let material = if self.weapon_mode {
            format!("arme : {}", self.weapons.selected().name())
        } else {
            format!("bloc : {}", block_display_name(self.selected_block))
        };
        let mouse_hint = if self.cursor_captured {
            "Échap : libérer la souris"
        } else {
            "clic : capturer la souris"
        };
        let pause_hint = if self.victory {
            "VICTOIRE | R recommencer | Échap quitter"
        } else if self.main_menu {
            "N nouvelle partie | C continuer | Échap quitter"
        } else if self.options_open {
            "OPTIONS | Flèches gauche/droite sensibilité | O fermer"
        } else if self.crafting_open {
            "FABRICATION | C briques | V planches | I fermer"
        } else if self.paused {
            "PAUSE | P reprendre | Échap quitter"
        } else {
            "P pause"
        };
        let audio_hint = self
            .audio_error
            .as_ref()
            .map_or("audio actif".to_owned(), |error| {
                format!("audio indisponible : {error}")
            });
        gl_window.window().set_title(&format!(
            "Voxel Frontier | {pause_hint} | WASD bouger | Espace sauter | F mêlée | Q arme suivante | 1-3 armes | E blocs | R recharger | clic gauche miner/tirer | clic droit poser | PV:{} | ennemis:{} | munitions:{}/{} | 1 Herbe:{} 2 Terre:{} 3 Pierre:{} 4 Brique:{} 5 Sable:{} 6 Neige:{} 7 Bois:{} 8 Feuilles:{} 9 Planches:{} | C 4 briques | V 4 planches | actif : {material} | F5/F9 sauvegarder/charger | {mouse_hint} | {} | {audio_hint}",
            self.player_health,
            self.mobs.iter().filter(|mob| mob.state != MobState::Dead).count(),
            self.weapons.ammo().0,
            self.weapons.ammo().1,
            self.inventory.count(BlockType::Grass),
            self.inventory.count(BlockType::Dirt),
            self.inventory.count(BlockType::Stone),
            self.inventory.count(BlockType::Brick),
            self.inventory.count(BlockType::Sand),
            self.inventory.count(BlockType::Snow),
            self.inventory.count(BlockType::Wood),
            self.inventory.count(BlockType::Leaves),
            self.inventory.count(BlockType::Planks),
            self.status,
        ));
    }

    fn play_sound(&self, effect: SoundEffect) {
        if let Some(audio) = &self.audio {
            audio.play(effect);
        }
    }

    /// Capture la souris pour la vue FPS, avec confinement comme solution de repli.
    fn capture_cursor(&mut self) -> Result<(), String> {
        let Some(gl_window) = &self.gl_window else {
            return Ok(());
        };
        let window = gl_window.window();
        window
            .set_cursor_grab(CursorGrabMode::Locked)
            .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined))
            .map_err(|err| format!("capture de la souris impossible : {err}"))?;
        window.set_cursor_visible(false);
        self.cursor_captured = true;
        self.update_window_title();
        Ok(())
    }

    /// Rend le curseur à l'utilisateur, notamment lorsque la fenêtre perd le focus.
    fn release_cursor(&mut self) -> Result<(), String> {
        let Some(gl_window) = &self.gl_window else {
            self.cursor_captured = false;
            return Ok(());
        };
        gl_window
            .window()
            .set_cursor_grab(CursorGrabMode::None)
            .map_err(|err| format!("libération de la souris impossible : {err}"))?;
        gl_window.window().set_cursor_visible(true);
        self.cursor_captured = false;
        self.update_window_title();
        Ok(())
    }

    fn restart_game(&mut self) -> Result<(), String> {
        save::clear_chunk_cache(&self.chunk_cache_path)
            .map_err(|err| format!("réinitialisation du cache impossible : {err}"))?;

        let mut dirty = self.world.chunk_coords();
        self.world = generate_area(WORLD_SEED, self.render_distance);
        self.camera = Camera {
            position: spawn_position(),
            pitch: -0.3,
            ..Camera::default()
        };
        self.vertical = VerticalState::default();
        self.last_stream_centers = None;
        self.mobs = vec![
            Mob::new(1, ai::spawn_position(WORLD_SEED, 8, 8)),
            Mob::new(2, ai::spawn_position(WORLD_SEED, -10, -8)),
        ];
        self.player_health = 10;
        self.player_damage_cooldown = 0.0;
        self.player_attack_cooldown = 0.0;
        self.inventory = Inventory::default();
        self.lighting = VoxelLighting::default();
        self.day_night = crate::lighting::DayNightCycle::default();
        self.selected_block = BlockType::Stone;
        self.weapons = WeaponInventory::default();
        self.weapon_mode = false;
        self.paused = false;
        self.crafting_open = false;
        self.options_open = false;
        self.victory = false;
        self.main_menu = false;
        self.input.clear();

        let center = split_block_pos(self.camera.position.floor().as_ivec3()).0;
        let added = ensure_chunks_around_radius(
            &mut self.world,
            center,
            self.render_distance,
            &self.chunk_cache_path,
        )?;
        self.last_stream_centers = None;
        dirty.extend(added);
        self.remesh(&dirty)?;
        self.status = "Nouvelle partie".to_owned();
        self.update_window_title();
        Ok(())
    }

    fn select_block(&mut self, key: KeyCode) {
        let slot = match key {
            KeyCode::Digit1 => Some(1),
            KeyCode::Digit2 => Some(2),
            KeyCode::Digit3 => Some(3),
            KeyCode::Digit4 => Some(4),
            KeyCode::Digit5 => Some(5),
            KeyCode::Digit6 => Some(6),
            KeyCode::Digit7 => Some(7),
            KeyCode::Digit8 => Some(8),
            KeyCode::Digit9 => Some(9),
            _ => None,
        };
        if let Some(block) = slot.and_then(block_for_hotbar_slot) {
            self.selected_block = block;
            self.weapon_mode = false;
            self.update_window_title();
        }
    }

    fn select_next_weapon(&mut self) {
        if self.weapon_mode {
            self.weapons.select_next();
        } else {
            self.weapon_mode = true;
        }
        let weapon = self.weapons.selected().name();
        self.status = format!("Arme sélectionnée : {weapon}");
        self.update_window_title();
    }

    fn select_weapon_slot(&mut self, slot: u8) {
        self.weapons.select_slot(slot.saturating_sub(1));
        self.weapon_mode = true;
        self.status = format!("Arme sélectionnée : {}", self.weapons.selected().name());
        self.update_window_title();
    }

    fn reload_weapon(&mut self) {
        self.weapon_mode = true;
        let loaded = self.weapons.reload();
        self.status = if loaded == 0 {
            "Rechargement impossible : chargeur plein ou réserve vide".to_owned()
        } else {
            format!("Rechargé : {loaded} munition(s)")
        };
        self.update_window_title();
    }

    /// Enregistre l'erreur de démarrage et demande l'arrêt de la boucle.
    fn fail(&mut self, event_loop: &ActiveEventLoop, message: String) {
        self.error = Some(message);
        event_loop.exit();
    }

    fn craft_bricks(&mut self) {
        match self.inventory.craft("stone_bricks") {
            Ok(()) => {
                self.status = "Fabriqué : 4 briques".to_owned();
                self.play_sound(SoundEffect::Craft);
            }
            Err(CraftError::MissingIngredients) => {
                self.status = "Fabrication impossible : il faut 2 pierres et 2 terres".to_owned()
            }
            Err(CraftError::InventoryFull) => {
                self.status = "Fabrication impossible : pile de briques pleine".to_owned()
            }
            Err(CraftError::UnknownRecipe) => self.status = "Recette introuvable".to_owned(),
            Err(CraftError::InvalidRecipeData(message)) => {
                self.status = format!("Recette invalide : {message}")
            }
        }
        self.update_window_title();
    }

    fn craft_planks(&mut self) {
        match self.inventory.craft("wood_planks") {
            Ok(()) => {
                self.status = "Fabriqué : 4 planches".to_owned();
                self.play_sound(SoundEffect::Craft);
            }
            Err(CraftError::MissingIngredients) => {
                self.status = "Fabrication impossible : il faut une bûche".to_owned()
            }
            Err(CraftError::InventoryFull) => {
                self.status = "Fabrication impossible : pile de planches pleine".to_owned()
            }
            Err(CraftError::UnknownRecipe) => self.status = "Recette introuvable".to_owned(),
            Err(CraftError::InvalidRecipeData(message)) => {
                self.status = format!("Recette invalide : {message}")
            }
        }
        self.update_window_title();
    }

    fn toggle_pause(&mut self) -> Result<(), String> {
        self.paused = !self.paused;
        self.crafting_open = false;
        self.options_open = false;
        self.input.clear();
        if self.paused && self.cursor_captured {
            self.release_cursor()?;
        }
        self.status = if self.paused {
            "Pause : P reprend, Échap quitte".to_owned()
        } else {
            "Partie reprise".to_owned()
        };
        self.update_window_title();
        Ok(())
    }

    fn toggle_crafting(&mut self) -> Result<(), String> {
        self.crafting_open = !self.crafting_open;
        self.options_open = false;
        self.input.clear();
        if self.crafting_open && self.cursor_captured {
            self.release_cursor()?;
        }
        self.status = if self.crafting_open {
            "Fabrication : C briques, V planches, I fermer".to_owned()
        } else {
            "Fabrication fermée".to_owned()
        };
        self.update_window_title();
        Ok(())
    }

    fn toggle_options(&mut self) -> Result<(), String> {
        self.options_open = !self.options_open;
        if self.options_open {
            self.crafting_open = false;
        }
        self.input.clear();
        if self.options_open && self.cursor_captured {
            self.release_cursor()?;
        }
        self.status = if self.options_open {
            "Options : gauche/droite sensibilité, haut/bas distance d'affichage".to_owned()
        } else {
            "Options fermées".to_owned()
        };
        self.update_window_title();
        Ok(())
    }

    fn adjust_mouse_sensitivity(&mut self, amount: i8) {
        let current = self.mouse_sensitivity_level.clamp(1, 5);
        let next = (i16::from(current) + i16::from(amount)).clamp(1, 5) as u8;
        if next == current {
            self.status = format!("Sensibilité souris : niveau {current}/5");
            self.update_window_title();
            return;
        }
        let settings = UserSettings {
            mouse_sensitivity_level: next,
            render_distance: self.render_distance,
        };
        match save_user_settings(Path::new(SETTINGS_PATH), &settings) {
            Ok(()) => {
                self.mouse_sensitivity_level = next;
                self.status = format!("Sensibilité souris : niveau {next}/5");
            }
            Err(error) => {
                self.status = error;
                eprintln!("{}", self.status);
            }
        }
        self.update_window_title();
    }

    fn adjust_render_distance(&mut self, amount: i32) {
        let next = (self.render_distance + amount).clamp(MIN_RENDER_DISTANCE, MAX_RENDER_DISTANCE);
        if next == self.render_distance {
            self.status = format!("Distance d'affichage : {next} chunks");
            self.update_window_title();
            return;
        }
        let settings = UserSettings {
            mouse_sensitivity_level: self.mouse_sensitivity_level,
            render_distance: next,
        };
        match save_user_settings(Path::new(SETTINGS_PATH), &settings) {
            Ok(()) => {
                self.render_distance = next;
                self.last_stream_centers = None;
                self.status = format!("Distance d'affichage : {next} chunks");
            }
            Err(error) => {
                self.status = error;
                eprintln!("{}", self.status);
            }
        }
        self.update_window_title();
    }

    fn attack(&mut self) {
        if self.player_attack_cooldown > 0.0 {
            self.status = "Attaque en récupération".to_owned();
            self.update_window_title();
            return;
        }
        self.player_attack_cooldown = PLAYER_ATTACK_INTERVAL;
        let target =
            ai::target_in_melee(&self.mobs, self.camera.position, self.camera.forward(), 2.5)
                .map(|mob| mob.id);
        let Some(target) = target else {
            self.status = "Aucun ennemi à portée".to_owned();
            self.update_window_title();
            return;
        };
        self.apply_player_hit(target, 1);
    }

    fn fire_weapon(&mut self) {
        let weapon = self.weapons.selected();
        if !self.weapons.fire() {
            let (loaded, reserve) = self.weapons.ammo();
            self.status = if loaded == 0 && reserve == 0 {
                "Plus de munitions : aucune réserve disponible".to_owned()
            } else if loaded == 0 {
                "Chargeur vide : appuyez sur R pour recharger".to_owned()
            } else {
                "Arme en récupération".to_owned()
            };
            self.update_window_title();
            return;
        }
        let target = ai::target_in_ray(
            &self.mobs,
            self.camera.position,
            self.camera.forward(),
            weapon.range(),
        )
        .map(|(mob, distance)| (mob.id, distance));
        let Some((target, target_distance)) = target else {
            self.status = format!("{} : tir manqué", weapon.name());
            self.update_window_title();
            return;
        };
        if raycast(
            &self.world,
            self.camera.position,
            self.camera.forward(),
            weapon.range(),
        )
        .is_some_and(|hit| hit.distance < target_distance)
        {
            self.status = format!("{} : tir bloqué par un bloc", weapon.name());
            self.update_window_title();
            return;
        }
        self.apply_player_hit(target, weapon.damage());
    }

    fn apply_player_hit(&mut self, target: u32, damage: u8) {
        let killed = self
            .mobs
            .iter_mut()
            .find(|mob| mob.id == target)
            .is_some_and(|mob| {
                if damage == 1 {
                    mob.take_hit()
                } else {
                    mob.take_damage(damage)
                }
            });
        self.play_sound(
            if killed && self.mobs.iter().all(|mob| mob.state == MobState::Dead) {
                SoundEffect::Victory
            } else {
                SoundEffect::Hit
            },
        );
        self.status = if killed {
            let reward = self.inventory.add(BlockType::Dirt, 2);
            self.victory = self.mobs.iter().all(|mob| mob.state == MobState::Dead);
            if self.victory {
                if reward {
                    "Victoire : zone nettoyée, 2 terres récupérées — R pour recommencer".to_owned()
                } else {
                    "Victoire : zone nettoyée (inventaire plein) — R pour recommencer".to_owned()
                }
            } else {
                if reward {
                    "Ennemi vaincu : 2 terres récupérées".to_owned()
                } else {
                    "Ennemi vaincu (inventaire plein)".to_owned()
                }
            }
        } else {
            "Ennemi touché".to_owned()
        };
        self.update_window_title();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gl_window.is_some() {
            return;
        }

        let attributes = Window::default_attributes().with_title("Voxel Frontier");
        let gl_window = match OpenGlWindow::new(event_loop, attributes) {
            Ok(gl_window) => gl_window,
            Err(err) => {
                self.fail(
                    event_loop,
                    format!("création de la fenêtre OpenGL impossible : {err}"),
                );
                return;
            }
        };

        let center = split_block_pos(self.camera.position.floor().as_ivec3()).0;
        if let Err(err) = ensure_chunks_around_radius(
            &mut self.world,
            center,
            self.render_distance,
            &self.chunk_cache_path,
        ) {
            self.fail(event_loop, err);
            return;
        }
        self.last_stream_centers = None;

        let renderer = match Renderer::new(gl_window.gl()) {
            Ok(renderer) => renderer,
            Err(err) => {
                self.fail(
                    event_loop,
                    format!("initialisation du rendu impossible : {err}"),
                );
                return;
            }
        };
        self.renderer = Some(renderer);
        self.gl_window = Some(gl_window);
        self.update_window_title();

        // Maillage initial de toute la zone générée.
        let coords = self.world.chunk_coords();
        if let Err(err) = self.remesh(&coords) {
            self.fail(event_loop, err);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.input.set_key(code, event.state);
                    // Une touche maintenue répète l'événement : seul le premier appui compte.
                    if event.state == ElementState::Pressed && !event.repeat {
                        match code {
                            KeyCode::Escape if self.cursor_captured => {
                                if let Err(err) = self.release_cursor() {
                                    self.fail(event_loop, err);
                                }
                            }
                            KeyCode::Escape if self.crafting_open => {
                                if let Err(err) = self.toggle_crafting() {
                                    self.fail(event_loop, err);
                                }
                            }
                            KeyCode::Escape if self.options_open => {
                                if let Err(err) = self.toggle_options() {
                                    self.fail(event_loop, err);
                                }
                            }
                            KeyCode::Escape => event_loop.exit(),
                            KeyCode::F5 if !self.main_menu && !self.victory => self.save_game(),
                            KeyCode::F9 if !self.paused && !self.main_menu => {
                                self.load_game();
                                if self.victory && self.cursor_captured {
                                    if let Err(err) = self.release_cursor() {
                                        self.fail(event_loop, err);
                                    }
                                }
                            }
                            KeyCode::KeyN if self.main_menu => {
                                if let Err(err) = self.restart_game() {
                                    self.fail(event_loop, err);
                                }
                            }
                            KeyCode::KeyC if self.main_menu => {
                                if Path::new(SAVE_PATH).is_file() {
                                    if self.load_game() && self.victory && self.cursor_captured {
                                        if let Err(err) = self.release_cursor() {
                                            self.fail(event_loop, err);
                                        }
                                    }
                                } else {
                                    self.status =
                                        format!("Aucune sauvegarde disponible : {SAVE_PATH}");
                                    self.update_window_title();
                                }
                            }
                            KeyCode::KeyC
                                if !self.paused
                                    && !self.options_open
                                    && !self.victory
                                    && !self.main_menu =>
                            {
                                self.craft_bricks()
                            }
                            KeyCode::KeyV
                                if !self.paused
                                    && !self.options_open
                                    && !self.victory
                                    && !self.main_menu =>
                            {
                                self.craft_planks()
                            }
                            KeyCode::KeyP if !self.victory && !self.main_menu => {
                                if let Err(err) = self.toggle_pause() {
                                    self.fail(event_loop, err);
                                }
                            }
                            KeyCode::KeyI if !self.paused && !self.victory && !self.main_menu => {
                                if !self.options_open {
                                    if let Err(err) = self.toggle_crafting() {
                                        self.fail(event_loop, err);
                                    }
                                }
                            }
                            KeyCode::KeyO if self.paused && !self.victory => {
                                if let Err(err) = self.toggle_options() {
                                    self.fail(event_loop, err);
                                }
                            }
                            KeyCode::ArrowLeft if self.options_open => {
                                self.adjust_mouse_sensitivity(-1);
                            }
                            KeyCode::ArrowRight if self.options_open => {
                                self.adjust_mouse_sensitivity(1);
                            }
                            KeyCode::ArrowUp if self.options_open => {
                                self.adjust_render_distance(1);
                            }
                            KeyCode::ArrowDown if self.options_open => {
                                self.adjust_render_distance(-1);
                            }
                            KeyCode::KeyR if self.victory => {
                                if let Err(err) = self.restart_game() {
                                    self.fail(event_loop, err);
                                }
                            }
                            KeyCode::KeyR
                                if !self.paused
                                    && !self.crafting_open
                                    && !self.options_open
                                    && !self.victory
                                    && !self.main_menu =>
                            {
                                self.reload_weapon()
                            }
                            KeyCode::KeyQ
                                if !self.paused
                                    && !self.crafting_open
                                    && !self.options_open
                                    && !self.victory
                                    && !self.main_menu =>
                            {
                                self.select_next_weapon()
                            }
                            KeyCode::KeyE
                                if !self.paused
                                    && !self.crafting_open
                                    && !self.options_open
                                    && !self.victory
                                    && !self.main_menu =>
                            {
                                self.weapon_mode = false;
                                self.status = "Mode construction".to_owned();
                                self.update_window_title();
                            }
                            KeyCode::KeyF
                                if !self.paused
                                    && !self.crafting_open
                                    && !self.options_open
                                    && !self.victory
                                    && !self.main_menu =>
                            {
                                self.attack();
                                if self.victory && self.cursor_captured {
                                    if let Err(err) = self.release_cursor() {
                                        self.fail(event_loop, err);
                                    }
                                }
                            }
                            KeyCode::Digit1
                            | KeyCode::Digit2
                            | KeyCode::Digit3
                            | KeyCode::Digit4
                            | KeyCode::Digit5
                            | KeyCode::Digit6
                                if !self.paused
                                    && !self.options_open
                                    && !self.victory
                                    && !self.main_menu =>
                            {
                                if self.weapon_mode {
                                    if let Some(slot) = weapon_slot_for_key(code) {
                                        self.select_weapon_slot(slot);
                                    } else {
                                        self.select_block(code);
                                    }
                                } else {
                                    self.select_block(code)
                                }
                            }
                            KeyCode::Digit7 | KeyCode::Digit8 | KeyCode::Digit9
                                if !self.paused
                                    && !self.options_open
                                    && !self.victory
                                    && !self.main_menu =>
                            {
                                self.select_block(code)
                            }
                            _ => {}
                        }
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if self.paused
                    || self.crafting_open
                    || self.options_open
                    || self.victory
                    || self.main_menu
                {
                    return;
                }
                self.input.set_mouse_button(button, state);
                if state == ElementState::Pressed {
                    if !self.cursor_captured {
                        if let Err(err) = self.capture_cursor() {
                            self.fail(event_loop, err);
                        }
                    } else if button == MouseButton::Left && self.weapon_mode {
                        self.fire_weapon();
                    } else if let Err(err) = self.edit_block(button) {
                        self.fail(event_loop, err);
                    }
                }
            }
            // Les relâchements survenus hors focus ne sont pas reçus : on repart d'un état vide.
            WindowEvent::Focused(false) => {
                self.input.clear();
                if self.cursor_captured {
                    if let Err(err) = self.release_cursor() {
                        self.fail(event_loop, err);
                    }
                }
            }
            WindowEvent::Resized(_) => {
                if let Some(gl_window) = &self.gl_window {
                    gl_window.resize();
                }
            }
            WindowEvent::RedrawRequested => self.render(event_loop),
            _ => {}
        }
    }

    fn device_event(&mut self, _event_loop: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if self.cursor_captured {
            if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
                self.input.add_mouse_motion(dx, dy);
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let elapsed = self.last_frame.map_or(Duration::ZERO, |last| now - last);
        self.last_frame = Some(now);

        for _ in 0..self.clock.advance(elapsed) {
            self.update();
        }
        if let Err(err) = self.stream_chunks() {
            self.fail(event_loop, err);
            return;
        }

        // Réveil à la prochaine échéance de pas fixe, sans tourner à vide en permanence.
        event_loop.set_control_flow(ControlFlow::WaitUntil(now + FIXED_STEP));
        if let Some(gl_window) = &self.gl_window {
            gl_window.window().request_redraw();
        }
    }
}

/// Lance l'application. Retourne une erreur si la boucle d'événements ou la fenêtre échoue.
pub fn run() -> Result<(), Box<dyn Error>> {
    println!("Voxel Frontier bootstrap");

    if std::env::args().any(|arg| arg == CHECK_FLAG) {
        return Ok(());
    }

    let settings = load_user_settings(Path::new(SETTINGS_PATH)).map_err(std::io::Error::other)?;
    let (audio, audio_error) = match AudioPlayer::new() {
        Ok(audio) => (Some(audio), None),
        Err(error) => {
            eprintln!("Initialisation audio impossible : {error}");
            (None, Some(error))
        }
    };
    let event_loop = EventLoop::new()?;
    let mut app = App {
        world: generate_area(WORLD_SEED, settings.render_distance),
        camera: Camera {
            position: spawn_position(),
            pitch: -0.3,
            ..Camera::default()
        },
        main_menu: true,
        mouse_sensitivity_level: settings.mouse_sensitivity_level,
        render_distance: settings.render_distance,
        audio,
        audio_error,
        mobs: vec![
            Mob::new(1, ai::spawn_position(WORLD_SEED, 8, 8)),
            Mob::new(2, ai::spawn_position(WORLD_SEED, -10, -8)),
        ],
        selected_block: BlockType::Stone,
        ..App::default()
    };
    event_loop.run_app(&mut app)?;

    match app.error {
        Some(message) => Err(message.into()),
        None => Ok(()),
    }
}

/// Position de départ déterministe : trois blocs au-dessus de la surface générée.
fn spawn_position() -> Vec3 {
    Vec3::new(0.5, surface_height(WORLD_SEED, 0, 0) as f32 + 3.0, 0.5)
}

fn fell_out_of_world(position: Vec3) -> bool {
    position.y < FALL_RESET_HEIGHT
}

fn resolve_player_damage(health: u8, cooldown: f32, incoming_damage: u8, dt: f32) -> (u8, f32) {
    let remaining_cooldown = if dt.is_finite() && dt > 0.0 {
        (cooldown - dt).max(0.0)
    } else {
        cooldown.max(0.0)
    };
    if incoming_damage == 0 || remaining_cooldown > 0.0 || health == 0 {
        return (health, remaining_cooldown);
    }

    (
        health.saturating_sub(1),
        if health > 1 {
            PLAYER_DAMAGE_COOLDOWN
        } else {
            0.0
        },
    )
}

fn new_chunk_cache_path() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "voxel_frontier_chunks_{}_{}",
        std::process::id(),
        timestamp
    ))
}

fn unload_radius(load_radius: i32) -> i32 {
    load_radius.saturating_add(1)
}

fn ensure_chunks_around_radius(
    world: &mut World,
    center: IVec3,
    radius: i32,
    cache_directory: &Path,
) -> Result<Vec<IVec3>, String> {
    let mut added = Vec::new();
    for x in center.x - radius..=center.x + radius {
        for z in center.z - radius..=center.z + radius {
            for y in 0..WORLD_CHUNK_LAYERS {
                let coord = IVec3::new(x, y, z);
                if world.chunk(coord).is_none() {
                    let chunk = save::load_cached_chunk(cache_directory, coord)
                        .map_err(|err| format!("chargement du chunk {coord} impossible : {err}"))?
                        .unwrap_or_else(|| generate_chunk(WORLD_SEED, coord));
                    world.insert_chunk(coord, chunk);
                    added.push(coord);
                }
            }
        }
    }
    Ok(added)
}

/// Écrit sur disque puis retire du monde les chunks hors du rayon conservé.
fn evict_chunks_outside_regions(
    world: &mut World,
    retained_regions: &[(IVec3, i32)],
    cache_directory: &Path,
) -> Result<Vec<IVec3>, String> {
    let distant: Vec<_> = world
        .chunk_coords()
        .into_iter()
        .filter(|coord| {
            !retained_regions.iter().any(|(center, radius)| {
                (coord.x - center.x).abs() <= *radius && (coord.z - center.z).abs() <= *radius
            })
        })
        .collect();
    for &coord in &distant {
        let chunk = world
            .chunk(coord)
            .expect("coordonnée issue des chunks chargés");
        save::cache_chunk(cache_directory, coord, chunk)
            .map_err(|err| format!("mise en cache du chunk {coord} impossible : {err}"))?;
        world.remove_chunk(coord);
    }
    Ok(distant)
}

fn block_for_hotbar_slot(slot: u8) -> Option<BlockType> {
    match slot {
        1 => Some(BlockType::Grass),
        2 => Some(BlockType::Dirt),
        3 => Some(BlockType::Stone),
        4 => Some(BlockType::Brick),
        5 => Some(BlockType::Sand),
        6 => Some(BlockType::Snow),
        7 => Some(BlockType::Wood),
        8 => Some(BlockType::Leaves),
        9 => Some(BlockType::Planks),
        _ => None,
    }
}

fn weapon_slot_for_key(key: KeyCode) -> Option<u8> {
    match key {
        KeyCode::Digit1 => Some(1),
        KeyCode::Digit2 => Some(2),
        KeyCode::Digit3 => Some(3),
        _ => None,
    }
}

fn can_craft_bricks(inventory: &Inventory) -> bool {
    inventory.count(BlockType::Stone) >= 2
        && inventory.count(BlockType::Dirt) >= 2
        && inventory.can_add(BlockType::Brick, 4)
}

fn can_craft_planks(inventory: &Inventory) -> bool {
    inventory.count(BlockType::Wood) >= 1 && inventory.can_add(BlockType::Planks, 4)
}

fn mouse_sensitivity(level: u8) -> f32 {
    BASE_MOUSE_SENSITIVITY * f32::from(level.clamp(1, 5)) / 3.0
}

fn block_display_name(block: BlockType) -> &'static str {
    match block {
        BlockType::Air => "air",
        BlockType::Grass => "herbe",
        BlockType::Dirt => "terre",
        BlockType::Stone => "pierre",
        BlockType::Brick => "brique",
        BlockType::Sand => "sable",
        BlockType::Snow => "neige",
        BlockType::Wood => "bois",
        BlockType::Leaves => "feuilles",
        BlockType::Planks => "planches",
    }
}

fn hotbar_slot_for_block(block: BlockType) -> u8 {
    match block {
        BlockType::Grass => 1,
        BlockType::Dirt => 2,
        BlockType::Stone => 3,
        BlockType::Brick => 4,
        BlockType::Sand => 5,
        BlockType::Snow => 6,
        BlockType::Wood => 7,
        BlockType::Leaves => 8,
        BlockType::Planks => 9,
        BlockType::Air => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voxel::chunk::{Chunk, CHUNK_SIZE};

    #[test]
    fn la_pause_suspend_la_simulation_et_efface_les_entrees() {
        let mut app = App::default();
        app.input.set_key(KeyCode::KeyW, ElementState::Pressed);
        app.toggle_pause().unwrap();
        let daylight = app.day_night.daylight();
        app.update();

        assert!(app.paused);
        assert!(!app.input.is_key_down(KeyCode::KeyW));
        assert_eq!(app.day_night.daylight(), daylight);

        app.toggle_pause().unwrap();
        assert!(!app.paused);
    }

    #[test]
    fn le_menu_principal_suspend_la_simulation() {
        let mut app = App {
            main_menu: true,
            world: floor_world(),
            ..App::default()
        };
        app.input.set_key(KeyCode::KeyW, ElementState::Pressed);
        let position = app.camera.position;
        let daylight = app.day_night.daylight();

        app.update();

        assert_eq!(app.camera.position, position);
        assert_eq!(app.day_night.daylight(), daylight);
        assert!(app.input.is_key_down(KeyCode::KeyW));
    }

    #[test]
    fn le_panneau_de_fabrication_suspend_la_partie_et_efface_les_entrees() {
        let mut app = App {
            crafting_open: true,
            world: floor_world(),
            ..App::default()
        };
        app.input.set_key(KeyCode::KeyW, ElementState::Pressed);
        let position = app.camera.position;
        let daylight = app.day_night.daylight();

        app.update();

        assert_eq!(app.camera.position, position);
        assert_eq!(app.day_night.daylight(), daylight);

        app.toggle_crafting().unwrap();
        assert!(!app.crafting_open);
        assert!(!app.input.is_key_down(KeyCode::KeyW));
    }

    #[test]
    fn la_pause_ferme_le_panneau_de_fabrication() {
        let mut app = App {
            crafting_open: true,
            ..App::default()
        };

        app.toggle_pause().unwrap();

        assert!(app.paused);
        assert!(!app.crafting_open);
    }

    #[test]
    fn les_options_s_ouvrent_depuis_la_pause_et_suspendent_le_jeu() {
        let mut app = App {
            paused: true,
            world: floor_world(),
            ..App::default()
        };
        app.input.set_key(KeyCode::KeyW, ElementState::Pressed);
        let position = app.camera.position;
        let daylight = app.day_night.daylight();

        app.toggle_options().unwrap();
        app.update();

        assert!(app.paused);
        assert!(app.options_open);
        assert_eq!(app.camera.position, position);
        assert_eq!(app.day_night.daylight(), daylight);

        app.mouse_sensitivity_level = 5;
        app.toggle_options().unwrap();
        assert!(app.paused);
        assert!(!app.options_open);
        assert_eq!(app.mouse_sensitivity_level, 5);
        assert!(!app.input.is_key_down(KeyCode::KeyW));
    }

    #[test]
    fn le_neuvieme_emplacement_selectionne_les_planches() {
        assert_eq!(block_for_hotbar_slot(9), Some(BlockType::Planks));
        assert_eq!(block_display_name(BlockType::Planks), "planches");
        assert_eq!(hotbar_slot_for_block(BlockType::Planks), 9);
        assert_eq!(hotbar_slot_for_block(BlockType::Air), 0);
    }

    #[test]
    fn les_recettes_ne_sont_disponibles_que_si_ingredients_et_sortie_sont_possibles() {
        let mut inventory = Inventory::default();
        inventory.add(BlockType::Stone, 2);
        inventory.add(BlockType::Dirt, 2);
        inventory.add(BlockType::Wood, 1);
        assert!(can_craft_bricks(&inventory));
        assert!(can_craft_planks(&inventory));

        inventory.add(BlockType::Brick, 999);
        inventory.add(BlockType::Planks, 999);
        assert!(!can_craft_bricks(&inventory));
        assert!(!can_craft_planks(&inventory));

        inventory.remove(BlockType::Stone, 1);
        inventory.remove(BlockType::Brick, 999);
        assert!(!can_craft_bricks(&inventory));
    }

    #[test]
    fn les_niveaux_de_sensibilite_sont_bornes_et_distincts() {
        assert_eq!(mouse_sensitivity(1), BASE_MOUSE_SENSITIVITY / 3.0);
        assert_eq!(mouse_sensitivity(3), BASE_MOUSE_SENSITIVITY);
        assert_eq!(mouse_sensitivity(5), BASE_MOUSE_SENSITIVITY * 5.0 / 3.0);
        assert_eq!(mouse_sensitivity(0), mouse_sensitivity(1));
        assert_eq!(mouse_sensitivity(6), mouse_sensitivity(5));
    }

    #[test]
    fn les_options_souris_sont_persistees_et_validees() {
        let path = std::env::temp_dir().join(format!(
            "voxel_frontier_settings_test_{}_{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        assert_eq!(
            load_user_settings(&path).unwrap().mouse_sensitivity_level,
            DEFAULT_MOUSE_SENSITIVITY_LEVEL
        );
        assert_eq!(
            load_user_settings(&path).unwrap().render_distance,
            DEFAULT_RENDER_DISTANCE
        );

        save_user_settings(
            &path,
            &UserSettings {
                mouse_sensitivity_level: 5,
                render_distance: MAX_RENDER_DISTANCE,
            },
        )
        .unwrap();
        assert_eq!(
            load_user_settings(&path).unwrap().mouse_sensitivity_level,
            5
        );
        assert_eq!(
            load_user_settings(&path).unwrap().render_distance,
            MAX_RENDER_DISTANCE
        );

        fs::write(&path, r#"{"mouse_sensitivity_level":0}"#).unwrap();
        assert!(load_user_settings(&path).is_err());
        fs::write(&path, r#"{"mouse_sensitivity_level":3}"#).unwrap();
        assert_eq!(
            load_user_settings(&path).unwrap().render_distance,
            DEFAULT_RENDER_DISTANCE
        );
        fs::write(
            &path,
            format!(
                r#"{{"mouse_sensitivity_level":3,"render_distance":{}}}"#,
                MAX_RENDER_DISTANCE + 1
            ),
        )
        .unwrap();
        assert!(load_user_settings(&path).is_err());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn un_pas_complet_produit_une_mise_a_jour() {
        let mut clock = FixedStepClock::default();

        assert_eq!(clock.advance(FIXED_STEP), 1);
    }

    #[test]
    fn un_temps_inferieur_au_pas_est_conserve() {
        let mut clock = FixedStepClock::default();

        assert_eq!(clock.advance(FIXED_STEP / 2), 0);
        assert_eq!(clock.advance(FIXED_STEP / 2 + Duration::from_micros(1)), 1);
    }

    #[test]
    fn le_rattrapage_est_borne() {
        let mut clock = FixedStepClock::default();
        let max_steps = (MAX_FRAME_TIME.as_micros() / FIXED_STEP.as_micros()) as u32;

        assert_eq!(clock.advance(Duration::from_secs(10)), max_steps);
    }

    #[test]
    fn les_attaques_simultanees_ne_causent_qu_un_degât_et_la_protection_expire() {
        let (health, cooldown) = resolve_player_damage(10, 0.0, 2, 0.0);
        assert_eq!(health, 9);
        assert_eq!(cooldown, PLAYER_DAMAGE_COOLDOWN);

        let (health, cooldown) = resolve_player_damage(health, cooldown, 1, 0.2);
        assert_eq!(health, 9);
        assert!((cooldown - 0.45).abs() < 1e-6);

        let (health, cooldown) = resolve_player_damage(health, cooldown, 1, 0.5);
        assert_eq!(health, 8);
        assert_eq!(cooldown, PLAYER_DAMAGE_COOLDOWN);
    }

    #[test]
    fn les_degats_n_affectent_pas_un_joueur_deja_vaincu() {
        assert_eq!(
            resolve_player_damage(0, 0.0, 1, FIXED_STEP.as_secs_f32()),
            (0, 0.0)
        );
    }

    #[test]
    fn les_attaques_du_joueur_respectent_un_intervalle_minimal() {
        let mut app = App {
            camera: Camera {
                position: Vec3::new(0.5, 2.6, 1.5),
                ..Camera::default()
            },
            world: floor_world(),
            mobs: vec![Mob::new(1, Vec3::new(0.5, 1.0, 0.5))],
            ..App::default()
        };

        app.attack();
        assert_eq!(app.status, "Ennemi touché");

        app.attack();
        assert_eq!(app.status, "Attaque en récupération");

        for _ in 0..20 {
            app.update();
        }
        app.attack();
        assert_eq!(app.status, "Ennemi touché");

        for _ in 0..20 {
            app.update();
        }
        app.attack();
        assert!(app.victory);
        assert!(app.status.starts_with("Victoire :"));
    }

    #[test]
    fn un_tir_touche_un_ennemi_et_consomme_une_munition() {
        let mut app = App {
            camera: Camera {
                position: Vec3::new(8.5, 2.3, 8.5),
                ..Camera::default()
            },
            world: floor_world(),
            mobs: vec![Mob::new(1, Vec3::new(8.5, 1.0, 5.5))],
            weapon_mode: true,
            ..App::default()
        };

        app.fire_weapon();

        assert_eq!(app.weapons.ammo(), (29, 120));
        assert_eq!(app.status, "Ennemi touché");
        assert_eq!(app.mobs[0].health(), 2);
    }

    #[test]
    fn le_fusil_a_pompe_peut_vaincre_un_ennemi_en_un_tir() {
        let mut app = App {
            camera: Camera {
                position: Vec3::new(8.5, 2.3, 8.5),
                ..Camera::default()
            },
            world: floor_world(),
            mobs: vec![Mob::new(1, Vec3::new(8.5, 1.0, 5.5))],
            weapon_mode: true,
            ..App::default()
        };
        app.weapons.select_slot(2);

        app.fire_weapon();

        assert!(app.victory);
        assert_eq!(app.mobs[0].health(), 0);
        assert_eq!(app.weapons.ammo(), (4, 30));
    }

    #[test]
    fn un_bloc_arrete_un_tir_et_ne_blesse_pas_l_ennemi() {
        let mut world = floor_world();
        assert!(world.set_block(IVec3::new(8, 2, 7), BlockType::Stone));
        let mut app = App {
            camera: Camera {
                position: Vec3::new(8.5, 2.3, 8.5),
                ..Camera::default()
            },
            world,
            mobs: vec![Mob::new(1, Vec3::new(8.5, 1.0, 5.5))],
            weapon_mode: true,
            ..App::default()
        };

        app.fire_weapon();

        assert_eq!(app.weapons.ammo(), (29, 120));
        assert_eq!(app.status, "RIFLE : tir bloqué par un bloc");
        assert_eq!(app.mobs[0].health(), 3);
    }

    #[test]
    fn les_touches_de_selection_distinguent_les_armes_et_les_blocs() {
        assert_eq!(weapon_slot_for_key(KeyCode::Digit1), Some(1));
        assert_eq!(weapon_slot_for_key(KeyCode::Digit3), Some(3));
        assert_eq!(weapon_slot_for_key(KeyCode::Digit4), None);

        let mut app = App::default();
        app.select_weapon_slot(3);
        assert_eq!(app.weapons.selected(), crate::weapons::WeaponKind::Shotgun);
        assert!(app.weapon_mode);

        app.select_block(KeyCode::Digit2);
        assert_eq!(app.selected_block, BlockType::Dirt);
        assert!(!app.weapon_mode);
    }

    #[test]
    fn recommencer_apres_victoire_reinitialise_la_partie() {
        let mut app = App {
            main_menu: true,
            victory: true,
            paused: true,
            player_health: 3,
            mobs: vec![Mob::new(1, Vec3::ZERO)],
            ..App::default()
        };
        app.mobs[0].mark_defeated();
        app.inventory.add(BlockType::Brick, 4);
        app.day_night.advance(60.0);

        app.restart_game().unwrap();

        assert!(!app.victory);
        assert!(!app.main_menu);
        assert!(!app.paused);
        assert_eq!(app.player_health, 10);
        assert_eq!(app.inventory, Inventory::default());
        assert_eq!(app.mobs.len(), 2);
        assert!(app.mobs.iter().all(|mob| mob.state != MobState::Dead));
        assert_eq!(app.selected_block, BlockType::Stone);
        assert!((app.day_night.daylight() - 1.0).abs() < 1e-6);
        assert_eq!(app.camera.position, spawn_position());
    }

    /// Sol plat : bloc de pierre à y = 0, surface à y = 1, dans le chunk (0, 0, 0).
    fn floor_world() -> World {
        let mut floor = Chunk::filled(BlockType::Air);
        for z in 0..CHUNK_SIZE as i32 {
            for x in 0..CHUNK_SIZE as i32 {
                floor.set(IVec3::new(x, 0, z), BlockType::Stone);
            }
        }
        let mut world = World::new();
        world.insert_chunk(IVec3::ZERO, floor);
        world
    }

    #[test]
    fn le_joueur_tombe_puis_reste_au_sol() {
        let world = floor_world();
        let dt = FIXED_STEP.as_secs_f32();
        let mut eye = Vec3::new(8.5, 5.0, 8.5);
        let mut vertical = VerticalState::default();

        for _ in 0..120 {
            eye += step_body(&world, eye, Vec3::ZERO, &mut vertical, false, dt);
        }

        assert!(vertical.on_ground);
        assert!((eye.y - (1.0 + EYE_HEIGHT)).abs() < 1e-3);
    }

    #[test]
    fn le_saut_culmine_pres_de_1_2_bloc() {
        let world = floor_world();
        let dt = FIXED_STEP.as_secs_f32();
        let mut eye = Vec3::new(8.5, 1.0 + EYE_HEIGHT, 8.5);
        let mut vertical = VerticalState {
            speed: 0.0,
            on_ground: true,
        };

        let start = eye.y;
        eye += step_body(&world, eye, Vec3::ZERO, &mut vertical, true, dt);
        let mut top = eye.y;
        for _ in 0..120 {
            eye += step_body(&world, eye, Vec3::ZERO, &mut vertical, false, dt);
            top = top.max(eye.y);
        }

        let rise = top - start;
        assert!(rise > 1.1 && rise < 1.4, "saut de {rise} blocs");
        assert!(vertical.on_ground);
    }

    #[test]
    fn une_chute_sous_le_monde_declenche_un_retour_au_depart() {
        assert!(fell_out_of_world(Vec3::new(
            0.5,
            FALL_RESET_HEIGHT - 0.1,
            0.5
        )));
        assert!(!fell_out_of_world(Vec3::new(0.5, FALL_RESET_HEIGHT, 0.5)));

        let spawn = spawn_position();
        assert_eq!(spawn.x, 0.5);
        assert_eq!(spawn.z, 0.5);
        assert!(spawn.y > FALL_RESET_HEIGHT);
    }

    #[test]
    fn les_emplacements_de_raccourci_selectionnent_des_blocs_constructibles() {
        assert_eq!(block_for_hotbar_slot(1), Some(BlockType::Grass));
        assert_eq!(block_for_hotbar_slot(2), Some(BlockType::Dirt));
        assert_eq!(block_for_hotbar_slot(3), Some(BlockType::Stone));
        assert_eq!(block_for_hotbar_slot(4), Some(BlockType::Brick));
        assert_eq!(block_for_hotbar_slot(5), Some(BlockType::Sand));
        assert_eq!(block_for_hotbar_slot(6), Some(BlockType::Snow));
        assert_eq!(block_for_hotbar_slot(7), Some(BlockType::Wood));
        assert_eq!(block_for_hotbar_slot(8), Some(BlockType::Leaves));
        assert_eq!(block_for_hotbar_slot(9), Some(BlockType::Planks));
        assert_eq!(block_for_hotbar_slot(0), None);
        assert_eq!(block_for_hotbar_slot(10), None);
        assert_eq!(block_display_name(BlockType::Sand), "sable");
        assert_eq!(block_display_name(BlockType::Snow), "neige");
    }

    #[test]
    fn les_chunks_initiaux_sont_generes_et_les_chunks_existants_conserves() {
        let center = IVec3::new(-3, 0, -1);
        let cache = std::env::temp_dir().join(format!(
            "voxel_frontier_initial_chunks_test_{}_cache",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&cache);
        let mut world = World::new();
        let preserved = Chunk::filled(BlockType::Brick);
        world.insert_chunk(center, preserved.clone());

        let added =
            ensure_chunks_around_radius(&mut world, center, WORLD_LOAD_RADIUS, &cache).unwrap();

        assert_eq!(
            added.len(),
            ((WORLD_LOAD_RADIUS * 2 + 1).pow(2) * WORLD_CHUNK_LAYERS) as usize - 1
        );
        assert_eq!(world.chunk(center), Some(&preserved));
        assert_eq!(
            world.chunk(IVec3::new(
                center.x - WORLD_LOAD_RADIUS,
                0,
                center.z - WORLD_LOAD_RADIUS
            )),
            Some(&generate_chunk(
                WORLD_SEED,
                IVec3::new(
                    center.x - WORLD_LOAD_RADIUS,
                    0,
                    center.z - WORLD_LOAD_RADIUS
                )
            ))
        );
        assert!(
            ensure_chunks_around_radius(&mut world, center, WORLD_LOAD_RADIUS, &cache)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            world.chunk_coords().len(),
            ((WORLD_LOAD_RADIUS * 2 + 1).pow(2) * WORLD_CHUNK_LAYERS) as usize
        );
        let _ = std::fs::remove_dir_all(cache);
    }

    #[test]
    fn le_rayonnement_des_chunks_suit_la_distance_de_rendu() {
        let cache = std::env::temp_dir().join(format!(
            "voxel_frontier_render_distance_test_{}_cache",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&cache);
        let mut world = World::new();

        ensure_chunks_around_radius(&mut world, IVec3::ZERO, MIN_RENDER_DISTANCE, &cache).unwrap();
        assert_eq!(
            world.chunk_coords().len(),
            ((MIN_RENDER_DISTANCE * 2 + 1).pow(2) * WORLD_CHUNK_LAYERS) as usize
        );

        ensure_chunks_around_radius(&mut world, IVec3::ZERO, MAX_RENDER_DISTANCE, &cache).unwrap();
        assert_eq!(
            world.chunk_coords().len(),
            ((MAX_RENDER_DISTANCE * 2 + 1).pow(2) * WORLD_CHUNK_LAYERS) as usize
        );
        let _ = std::fs::remove_dir_all(cache);
    }

    #[test]
    fn les_chunks_lointains_sont_evinces_et_restaures_depuis_le_cache() {
        let cache = std::env::temp_dir().join(format!(
            "voxel_frontier_streaming_test_{}_chunks",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&cache);
        let near = IVec3::ZERO;
        let far = IVec3::new(-4, 1, 2);
        let chunk = Chunk::filled(BlockType::Snow);
        let mut world = World::new();
        world.insert_chunk(near, Chunk::filled(BlockType::Grass));
        world.insert_chunk(far, chunk.clone());

        assert_eq!(
            evict_chunks_outside_regions(&mut world, &[(IVec3::ZERO, 2)], &cache).unwrap(),
            vec![far]
        );
        assert!(world.chunk(far).is_none());
        assert_eq!(save::load_cached_chunk(&cache, far).unwrap(), Some(chunk));
        assert!(std::fs::read_dir(&cache).unwrap().next().is_none());

        let _ = std::fs::remove_dir_all(cache);
    }

    #[test]
    fn le_streaming_garde_les_chunks_autour_des_ennemis_actifs() {
        let cache = std::env::temp_dir().join(format!(
            "voxel_frontier_mob_streaming_test_{}_cache",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&cache);
        let mut app = App {
            mobs: vec![Mob::new(1, Vec3::new(80.5, 1.0, 8.5))],
            chunk_cache_path: cache.clone(),
            ..App::default()
        };

        app.stream_chunks().unwrap();

        assert!(app.world.chunk(IVec3::new(5, 0, 0)).is_some());
        assert!(app.world.chunk(IVec3::new(6, 0, 0)).is_some());
        assert!(app.world.chunk(IVec3::new(7, 0, 0)).is_none());
        assert_eq!(
            app.world.chunk_coords().len(),
            ((WORLD_LOAD_RADIUS * 2 + 1).pow(2) + 3 * 3) as usize * WORLD_CHUNK_LAYERS as usize
        );

        let _ = std::fs::remove_dir_all(cache);
    }

    #[test]
    fn le_streaming_borne_les_chunks_charges_et_restaure_les_modifications() {
        let unload_distance = unload_radius(WORLD_LOAD_RADIUS);
        let cache = std::env::temp_dir().join(format!(
            "voxel_frontier_streaming_bound_{}_cache",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&cache);
        let mut world = World::new();
        world.insert_chunk(IVec3::ZERO, Chunk::filled(BlockType::Brick));
        ensure_chunks_around_radius(&mut world, IVec3::ZERO, WORLD_LOAD_RADIUS, &cache).unwrap();

        let distant_center = IVec3::new(20, 0, 20);
        ensure_chunks_around_radius(&mut world, distant_center, WORLD_LOAD_RADIUS, &cache).unwrap();
        evict_chunks_outside_regions(&mut world, &[(distant_center, unload_distance)], &cache)
            .unwrap();
        assert_eq!(
            world.chunk_coords().len(),
            ((unload_distance * 2 + 1).pow(2) * WORLD_CHUNK_LAYERS) as usize
        );
        assert!(world.chunk_coords().iter().all(|coord| {
            (coord.x - distant_center.x).abs() <= unload_distance
                && (coord.z - distant_center.z).abs() <= unload_distance
        }));

        ensure_chunks_around_radius(&mut world, IVec3::ZERO, WORLD_LOAD_RADIUS, &cache).unwrap();
        evict_chunks_outside_regions(&mut world, &[(IVec3::ZERO, unload_distance)], &cache)
            .unwrap();
        assert_eq!(
            world
                .chunk(IVec3::ZERO)
                .and_then(|chunk| chunk.get(IVec3::ZERO)),
            Some(BlockType::Brick)
        );
        assert_eq!(
            world.chunk_coords().len(),
            ((unload_distance * 2 + 1).pow(2) * WORLD_CHUNK_LAYERS) as usize
        );

        let _ = std::fs::remove_dir_all(cache);
    }
}
