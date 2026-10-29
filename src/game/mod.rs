use std::error::Error;
use std::path::Path;
use std::time::{Duration, Instant};

use glam::{IVec3, Vec3};
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::camera::Camera;
use crate::generation::{generate_area, surface_height};
use crate::input::InputState;
use crate::physics::collision::{move_aabb, Aabb};
use crate::render::{OpenGlWindow, Renderer};
use crate::save;
use crate::voxel::block::BlockType;
use crate::voxel::mesh::build_chunk_mesh;
use crate::voxel::raycast::raycast;
use crate::voxel::world::{chunks_touching, World};

/// Pas de temps fixe de la simulation (environ 60 Hz).
const FIXED_STEP: Duration = Duration::from_micros(16_667);

/// Temps réel maximal rattrapé en une seule image, pour éviter une spirale de mises à jour après une pause.
const MAX_FRAME_TIME: Duration = Duration::from_millis(250);

/// Argument de ligne de commande : affiche le message de démarrage et quitte sans ouvrir de fenêtre.
const CHECK_FLAG: &str = "--check";

/// Radians de rotation par unité de mouvement brut de souris.
const MOUSE_SENSITIVITY: f32 = 0.002;

/// Graine du monde : une même graine redonne le même terrain.
const WORLD_SEED: u64 = 2026;

/// Rayon de la zone générée, en chunks.
const WORLD_RADIUS: i32 = 2;

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

/// Bloc posé par le clic droit.
const PLACED_BLOCK: BlockType = BlockType::Stone;

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
#[derive(Default)]
struct App {
    gl_window: Option<OpenGlWindow>,
    renderer: Option<Renderer>,
    clock: FixedStepClock,
    last_frame: Option<Instant>,
    input: InputState,
    camera: Camera,
    vertical: VerticalState,
    world: World,
    error: Option<String>,
}

impl App {
    /// Pas de simulation : applique la souris, puis le déplacement borné par les collisions.
    fn update(&mut self) {
        let (dx, dy) = self.input.take_mouse_delta();
        // Souris vers la droite : yaw décroissant ; vers le bas : pitch décroissant.
        self.camera.rotate(
            -dx as f32 * MOUSE_SENSITIVITY,
            -dy as f32 * MOUSE_SENSITIVITY,
        );

        let dt = FIXED_STEP.as_secs_f32();
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
        renderer.draw(gl_window.gl(), size.width, size.height, view_proj);

        if let Err(err) = gl_window.swap_buffers() {
            self.fail(
                event_loop,
                format!("affichage de l'image impossible : {err}"),
            );
        }
    }

    /// Clic gauche : retire le bloc visé. Clic droit : pose de la pierre sur la face visée.
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
            MouseButton::Left => (hit.block, BlockType::Air),
            MouseButton::Right => (hit.block + hit.normal, PLACED_BLOCK),
            _ => return Ok(()),
        };

        // Interdit de poser un bloc dans la boîte du joueur.
        if block.is_solid() && player_aabb(self.camera.position).overlaps_cell(cell) {
            return Ok(());
        }
        if !self.world.set_block(cell, block) {
            return Ok(());
        }
        self.remesh(&chunks_touching(cell))
    }

    /// Reconstruit le maillage GPU des chunks donnés à partir de l'état du monde.
    fn remesh(&mut self, coords: &[IVec3]) -> Result<(), String> {
        let (Some(gl_window), Some(renderer)) = (&self.gl_window, &mut self.renderer) else {
            return Ok(());
        };
        for &coord in coords {
            let mesh = build_chunk_mesh(&self.world, coord);
            renderer.set_chunk_mesh(gl_window.gl(), coord, &mesh)?;
        }
        Ok(())
    }

    /// Écrit le monde dans `SAVE_PATH`. Une erreur est affichée sans arrêter le jeu.
    fn save_game(&self) {
        match save::save_world(Path::new(SAVE_PATH), &self.world) {
            Ok(()) => println!("Monde sauvegardé dans {SAVE_PATH}"),
            Err(err) => eprintln!("Sauvegarde impossible : {err}"),
        }
    }

    /// Remplace le monde par celui de `SAVE_PATH`. En cas d'erreur, le monde courant est conservé.
    fn load_game(&mut self) {
        let world = match save::load_world(Path::new(SAVE_PATH)) {
            Ok(world) => world,
            Err(err) => {
                eprintln!("Chargement impossible : {err}");
                return;
            }
        };
        // Les chunks absents du monde chargé doivent disparaître de l'affichage.
        let mut coords = self.world.chunk_coords();
        self.world = world;
        coords.extend(self.world.chunk_coords());
        if let Err(err) = self.remesh(&coords) {
            eprintln!("Remaillage impossible : {err}");
        }
    }

    /// Enregistre l'erreur de démarrage et demande l'arrêt de la boucle.
    fn fail(&mut self, event_loop: &ActiveEventLoop, message: String) {
        self.error = Some(message);
        event_loop.exit();
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
                            KeyCode::Escape => event_loop.exit(),
                            KeyCode::F5 => self.save_game(),
                            KeyCode::F9 => self.load_game(),
                            _ => {}
                        }
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.input.set_mouse_button(button, state);
                if state == ElementState::Pressed {
                    if let Err(err) = self.edit_block(button) {
                        self.fail(event_loop, err);
                    }
                }
            }
            // Les relâchements survenus hors focus ne sont pas reçus : on repart d'un état vide.
            WindowEvent::Focused(false) => self.input.clear(),
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
        if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            self.input.add_mouse_motion(dx, dy);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let elapsed = self.last_frame.map_or(Duration::ZERO, |last| now - last);
        self.last_frame = Some(now);

        for _ in 0..self.clock.advance(elapsed) {
            self.update();
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

    let event_loop = EventLoop::new()?;
    // Apparition 3 blocs au-dessus de la surface en (0, 0) : le joueur n'est pas dans un bloc.
    let spawn = Vec3::new(0.5, surface_height(WORLD_SEED, 0, 0) as f32 + 3.0, 0.5);
    let mut app = App {
        camera: Camera {
            position: spawn,
            pitch: -0.3,
            ..Camera::default()
        },
        world: generate_area(WORLD_SEED, WORLD_RADIUS),
        ..App::default()
    };
    event_loop.run_app(&mut app)?;

    match app.error {
        Some(message) => Err(message.into()),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voxel::chunk::{Chunk, CHUNK_SIZE};

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
}
