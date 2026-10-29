//! Rendu OpenGL 3.3 : un maillage GPU par chunk, dessiné avec la matrice projection * vue.

mod context;

use std::collections::HashMap;

use glam::{IVec3, Mat4};
use glow::HasContext;

pub use context::OpenGlWindow;

use crate::voxel::mesh::{MeshData, Vertex};

#[derive(Clone, Copy, Debug, Default)]
pub struct HudState {
    pub player_health: u8,
    pub enemies_remaining: usize,
    pub selected_hotbar_slot: u8,
    pub hotbar_counts: [u16; 9],
    pub selected_weapon_slot: u8,
    pub weapon_ammo: u8,
    pub weapon_reserve: u16,
    pub paused: bool,
    pub victory: bool,
    pub main_menu: bool,
    pub can_continue: bool,
    pub crafting_open: bool,
    pub can_craft_bricks: bool,
    pub can_craft_planks: bool,
    pub stone_count: u16,
    pub dirt_count: u16,
    pub wood_count: u16,
    pub options_open: bool,
    pub mouse_sensitivity_level: u8,
    pub render_distance: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct SceneLighting {
    pub ambient: f32,
    pub sky_color: [f32; 3],
    pub sun_direction: [f32; 3],
}

#[derive(Clone, Copy, Debug)]
pub struct FrameState {
    pub hud: HudState,
    pub lighting: SceneLighting,
}

const VERTEX_SHADER: &str = include_str!("../../assets/shaders/cube.vert");
const FRAGMENT_SHADER: &str = include_str!("../../assets/shaders/cube.frag");

/// Pas entre deux sommets, en octets. `Vertex` est `repr(C)` : pas de remplissage.
const STRIDE: i32 = std::mem::size_of::<Vertex>() as i32;

/// Décalage de la couleur dans un sommet, en octets (après la position).
const COLOR_OFFSET: i32 = std::mem::size_of::<[f32; 3]>() as i32;
const NORMAL_OFFSET: i32 = COLOR_OFFSET * 2;

/// Maillage d'un chunk sur le GPU : VAO, tampons et nombre d'index à dessiner.
struct GpuMesh {
    vertex_array: glow::VertexArray,
    vertex_buffer: glow::Buffer,
    index_buffer: glow::Buffer,
    index_count: i32,
}

impl GpuMesh {
    /// Libère les ressources GL. Le contexte OpenGL doit être courant.
    fn delete(self, gl: &glow::Context) {
        // SAFETY: le contexte est courant sur ce thread (voir `Renderer::new`).
        unsafe {
            gl.delete_vertex_array(self.vertex_array);
            gl.delete_buffer(self.vertex_buffer);
            gl.delete_buffer(self.index_buffer);
        }
    }
}

/// Programme de rendu et maillages GPU, indexés par coordonnée de chunk.
pub struct Renderer {
    program: glow::Program,
    view_proj_location: Option<glow::UniformLocation>,
    model_location: Option<glow::UniformLocation>,
    ambient_location: Option<glow::UniformLocation>,
    sun_direction_location: Option<glow::UniformLocation>,
    meshes: HashMap<IVec3, GpuMesh>,
    entity_mesh: GpuMesh,
}

impl Renderer {
    /// Compile les shaders et prépare l'état GL. Le contexte OpenGL doit être courant.
    pub fn new(gl: &glow::Context) -> Result<Self, String> {
        // SAFETY: le contexte est courant sur ce thread (garanti par `OpenGlWindow::new`).
        unsafe {
            let program = compile_program(gl)?;
            let view_proj_location = gl.get_uniform_location(program, "u_view_proj");
            let model_location = gl.get_uniform_location(program, "u_model");
            let ambient_location = gl.get_uniform_location(program, "u_ambient");
            let sun_direction_location = gl.get_uniform_location(program, "u_sun_direction");
            let entity_mesh = upload_mesh(
                gl,
                &crate::voxel::mesh::build_entity_cube_mesh([0.85, 0.12, 0.08]),
            )?;

            // Sans test de profondeur, les faces arrière se dessineraient par-dessus les avant.
            gl.enable(glow::DEPTH_TEST);
            // Seules les faces antihoraires vues de l'extérieur sont dessinées (voir `voxel::mesh`).
            gl.enable(glow::CULL_FACE);
            gl.clear_color(0.05, 0.05, 0.08, 1.0);

            Ok(Self {
                program,
                view_proj_location,
                model_location,
                ambient_location,
                sun_direction_location,
                meshes: HashMap::new(),
                entity_mesh,
            })
        }
    }

    /// Remplace le maillage GPU du chunk `coord`. Un maillage vide retire le chunk de l'affichage.
    pub fn set_chunk_mesh(
        &mut self,
        gl: &glow::Context,
        coord: IVec3,
        mesh: &MeshData,
    ) -> Result<(), String> {
        if let Some(old) = self.meshes.remove(&coord) {
            old.delete(gl);
        }
        if !mesh.indices.is_empty() {
            let gpu = upload_mesh(gl, mesh)?;
            self.meshes.insert(coord, gpu);
        }
        Ok(())
    }

    /// Dessine tous les chunks dans une zone de `width` x `height` pixels avec la matrice `view_proj`.
    /// Les dimensions doivent être non nulles et correspondre à la surface courante.
    pub fn draw(
        &self,
        gl: &glow::Context,
        width: u32,
        height: u32,
        view_proj: Mat4,
        entities: &[(glam::Vec3, glam::Vec3)],
        frame: FrameState,
    ) {
        // SAFETY: le contexte est courant sur ce thread (voir `Renderer::new`).
        unsafe {
            gl.viewport(0, 0, width as i32, height as i32);
            gl.clear_color(
                frame.lighting.sky_color[0],
                frame.lighting.sky_color[1],
                frame.lighting.sky_color[2],
                1.0,
            );
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);

            gl.use_program(Some(self.program));
            gl.uniform_1_f32(
                self.ambient_location.as_ref(),
                frame.lighting.ambient.clamp(0.0, 1.0),
            );
            gl.uniform_3_f32(
                self.sun_direction_location.as_ref(),
                frame.lighting.sun_direction[0],
                frame.lighting.sun_direction[1],
                frame.lighting.sun_direction[2],
            );
            gl.uniform_matrix_4_f32_slice(
                self.view_proj_location.as_ref(),
                false,
                &view_proj.to_cols_array(),
            );
            gl.uniform_matrix_4_f32_slice(
                self.model_location.as_ref(),
                false,
                &Mat4::IDENTITY.to_cols_array(),
            );

            for mesh in self.meshes.values() {
                gl.bind_vertex_array(Some(mesh.vertex_array));
                gl.draw_elements(glow::TRIANGLES, mesh.index_count, glow::UNSIGNED_INT, 0);
            }
            gl.bind_vertex_array(Some(self.entity_mesh.vertex_array));
            for &(position, scale) in entities {
                let model =
                    Mat4::from_scale_rotation_translation(scale, glam::Quat::IDENTITY, position);
                gl.uniform_matrix_4_f32_slice(
                    self.model_location.as_ref(),
                    false,
                    &model.to_cols_array(),
                );
                gl.draw_elements(
                    glow::TRIANGLES,
                    self.entity_mesh.index_count,
                    glow::UNSIGNED_INT,
                    0,
                );
            }
        }
        draw_hud(gl, width, height, frame.hud, frame.lighting.sky_color);
    }
}

fn draw_hud(gl: &glow::Context, width: u32, height: u32, hud: HudState, sky_color: [f32; 3]) {
    let width = width.min(i32::MAX as u32) as i32;
    let height = height.min(i32::MAX as u32) as i32;
    if width == 0 || height == 0 {
        return;
    }

    let padding = 12.min(width / 2);
    let meter_width = (width - padding * 2).min(180);
    let fill_width = meter_fill_width(meter_width, hud.player_health, 10);
    let y = padding;
    // SAFETY: the OpenGL context is current, as required by `Renderer::draw`.
    unsafe {
        gl.enable(glow::SCISSOR_TEST);
        draw_hud_rect(gl, padding, y, meter_width, 10, [0.24, 0.08, 0.08, 1.0]);
        draw_hud_rect(gl, padding, y, fill_width, 10, [0.16, 0.82, 0.28, 1.0]);

        let enemy_count = hud.enemies_remaining.min(10) as i32;
        for index in 0..enemy_count {
            draw_hud_rect(
                gl,
                width - padding - 12 - index * 17,
                y,
                12,
                10,
                [0.9, 0.16, 0.1, 1.0],
            );
        }

        draw_hotbar(
            gl,
            width,
            height,
            hud.selected_hotbar_slot,
            hud.hotbar_counts,
        );
        if hud.selected_weapon_slot > 0 {
            draw_weapon_status(gl, width, height, hud);
        }

        let center_x = width / 2;
        let center_y = height / 2;
        if hud.main_menu {
            draw_main_menu(gl, width, height, hud.can_continue);
        } else if hud.victory {
            draw_victory_overlay(gl, width, height);
        } else if hud.crafting_open {
            draw_crafting_overlay(gl, width, height, hud);
        } else if hud.options_open {
            draw_options_overlay(
                gl,
                width,
                height,
                hud.mouse_sensitivity_level,
                hud.render_distance,
            );
        } else if hud.paused {
            draw_pause_overlay(gl, width, height);
        } else {
            draw_hud_rect(gl, center_x - 7, center_y, 14, 2, [1.0, 1.0, 1.0, 0.9]);
            draw_hud_rect(gl, center_x, center_y - 7, 2, 14, [1.0, 1.0, 1.0, 0.9]);
        }
        gl.disable(glow::SCISSOR_TEST);
        gl.clear_color(sky_color[0], sky_color[1], sky_color[2], 1.0);
    }
}

unsafe fn draw_weapon_status(gl: &glow::Context, width: i32, height: i32, hud: HudState) {
    let Some(weapon) =
        crate::weapons::WeaponKind::ALL.get(hud.selected_weapon_slot.saturating_sub(1) as usize)
    else {
        return;
    };
    let text = format!(
        "{} {}/{}",
        weapon.name(),
        hud.weapon_ammo,
        hud.weapon_reserve
    );
    let text_width = bitmap_text_width(&text, 1);
    let y = 26 + hotbar_slot_size(width, 12) + 10;
    if height < y + 7 {
        return;
    }
    draw_bitmap_text(
        gl,
        (width - text_width) / 2,
        y,
        1,
        &text,
        [0.98, 0.88, 0.43, 1.0],
    );
}

/// Draws the pause state and the available keyboard actions.
///
/// # Safety
/// The OpenGL context must be current on the calling thread.
unsafe fn draw_pause_overlay(gl: &glow::Context, width: i32, height: i32) {
    let panel_width = width.min(264);
    let panel_height = height.min(144);
    let x = (width - panel_width) / 2;
    let y = (height - panel_height) / 2;
    let compact = panel_width < 200 || panel_height < 100;
    let scale = if compact { 1 } else { 2 };
    draw_hud_rect(gl, x, y, panel_width, panel_height, [0.2, 0.46, 0.72, 1.0]);
    draw_hud_rect(
        gl,
        x + 3,
        y + 3,
        panel_width - 6,
        panel_height - 6,
        [0.025, 0.04, 0.055, 1.0],
    );
    draw_bitmap_text(
        gl,
        x + (panel_width - bitmap_text_width("PAUSED", scale)) / 2,
        y + if compact { 16 } else { 22 },
        scale,
        "PAUSED",
        [0.55, 0.82, 1.0, 1.0],
    );
    draw_bitmap_text(
        gl,
        x + (panel_width - bitmap_text_width("P - RESUME", 1)) / 2,
        y + if compact { 48 } else { 68 },
        1,
        "P - RESUME",
        [0.95, 0.95, 0.95, 1.0],
    );
    draw_bitmap_text(
        gl,
        x + (panel_width - bitmap_text_width("ESC - QUIT", 1)) / 2,
        y + if compact { 64 } else { 84 },
        1,
        "ESC - QUIT",
        [0.75, 0.78, 0.84, 1.0],
    );
    draw_bitmap_text(
        gl,
        x + (panel_width - bitmap_text_width("O - OPTIONS", 1)) / 2,
        y + if compact { 78 } else { 100 },
        1,
        "O - OPTIONS",
        [0.75, 0.78, 0.84, 1.0],
    );
}

/// Draws the mouse sensitivity option and keyboard instructions.
///
/// # Safety
/// The OpenGL context must be current on the calling thread.
unsafe fn draw_options_overlay(
    gl: &glow::Context,
    width: i32,
    height: i32,
    level: u8,
    render_distance: i32,
) {
    let panel_width = width.min(280);
    let panel_height = height.min(190);
    let x = (width - panel_width) / 2;
    let y = (height - panel_height) / 2;
    draw_hud_rect(gl, x, y, panel_width, panel_height, [0.2, 0.46, 0.72, 1.0]);
    draw_hud_rect(
        gl,
        x + 3,
        y + 3,
        panel_width - 6,
        panel_height - 6,
        [0.025, 0.04, 0.055, 1.0],
    );
    draw_bitmap_text(
        gl,
        x + (panel_width - bitmap_text_width("OPTIONS", 2)) / 2,
        y + 18,
        2,
        "OPTIONS",
        [0.55, 0.82, 1.0, 1.0],
    );
    draw_bitmap_text(
        gl,
        x + (panel_width - bitmap_text_width("MOUSE SENSITIVITY", 1)) / 2,
        y + 50,
        1,
        "MOUSE SENSITIVITY",
        [0.9, 0.92, 0.96, 1.0],
    );
    const INDICATOR_WIDTH: i32 = 18;
    const GAP: i32 = 8;
    let row_width = INDICATOR_WIDTH * 5 + GAP * 4;
    let start_x = x + (panel_width - row_width) / 2;
    for index in 0_i32..5 {
        let selected = index as u8 + 1 == level.clamp(1, 5);
        draw_hud_rect(
            gl,
            start_x + index * (INDICATOR_WIDTH + GAP),
            y + 68,
            INDICATOR_WIDTH,
            10,
            if selected {
                [0.95, 0.78, 0.25, 1.0]
            } else {
                [0.25, 0.29, 0.36, 1.0]
            },
        );
    }
    draw_bitmap_text(
        gl,
        x + (panel_width - bitmap_text_width("LEFT / RIGHT SENSITIVITY", 1)) / 2,
        y + 91,
        1,
        "LEFT / RIGHT SENSITIVITY",
        [0.78, 0.82, 0.88, 1.0],
    );
    draw_bitmap_text(
        gl,
        x + (panel_width - bitmap_text_width("VIEW DISTANCE", 1)) / 2,
        y + 111,
        1,
        "VIEW DISTANCE",
        [0.9, 0.92, 0.96, 1.0],
    );
    let distance_row_width = INDICATOR_WIDTH * 3 + GAP * 2;
    let distance_start_x = x + (panel_width - distance_row_width) / 2;
    for index in 0_i32..3 {
        draw_hud_rect(
            gl,
            distance_start_x + index * (INDICATOR_WIDTH + GAP),
            y + 128,
            INDICATOR_WIDTH,
            10,
            if index + 1 == render_distance.clamp(1, 3) {
                [0.95, 0.78, 0.25, 1.0]
            } else {
                [0.25, 0.29, 0.36, 1.0]
            },
        );
    }
    draw_bitmap_text(
        gl,
        x + (panel_width - bitmap_text_width("UP / DOWN", 1)) / 2,
        y + 145,
        1,
        "UP / DOWN",
        [0.78, 0.82, 0.88, 1.0],
    );
    draw_bitmap_text(
        gl,
        x + (panel_width - bitmap_text_width("O - CLOSE", 1)) / 2,
        y + 168,
        1,
        "O - CLOSE",
        [0.75, 0.78, 0.84, 1.0],
    );
}

/// Draws keyboard recipes and current ingredients in a compact panel.
///
/// # Safety
/// The OpenGL context must be current on the calling thread.
unsafe fn draw_crafting_overlay(gl: &glow::Context, width: i32, height: i32, hud: HudState) {
    let panel_width = width.min(300);
    let panel_height = height.min(188);
    let x = (width - panel_width) / 2;
    let y = (height - panel_height) / 2;
    let compact = panel_width < 220 || panel_height < 150;
    let scale = if compact { 1 } else { 2 };
    let line_gap = if compact { 13 } else { 20 };
    let title_y = y + if compact { 10 } else { 16 };
    let first_y = y + if compact { 32 } else { 54 };
    draw_hud_rect(gl, x, y, panel_width, panel_height, [0.2, 0.46, 0.72, 1.0]);
    draw_hud_rect(
        gl,
        x + 3,
        y + 3,
        panel_width - 6,
        panel_height - 6,
        [0.025, 0.04, 0.055, 1.0],
    );
    draw_bitmap_text(
        gl,
        x + (panel_width - bitmap_text_width("CRAFTING", scale)) / 2,
        title_y,
        scale,
        "CRAFTING",
        [0.55, 0.82, 1.0, 1.0],
    );
    draw_bitmap_text(
        gl,
        x + 12,
        first_y,
        1,
        "C - 4 BRICKS",
        recipe_color(hud.can_craft_bricks),
    );
    draw_bitmap_text(
        gl,
        x + 12,
        first_y + line_gap,
        1,
        &format!("STONE {} + DIRT {}", hud.stone_count, hud.dirt_count),
        [0.78, 0.82, 0.88, 1.0],
    );
    draw_bitmap_text(
        gl,
        x + 12,
        first_y + line_gap * 2,
        1,
        "V - 4 PLANKS",
        recipe_color(hud.can_craft_planks),
    );
    draw_bitmap_text(
        gl,
        x + 12,
        first_y + line_gap * 3,
        1,
        &format!("WOOD {}", hud.wood_count),
        [0.78, 0.82, 0.88, 1.0],
    );
    draw_bitmap_text(
        gl,
        x + (panel_width - bitmap_text_width("I - CLOSE", 1)) / 2,
        y + panel_height - 20,
        1,
        "I - CLOSE",
        [0.75, 0.78, 0.84, 1.0],
    );
}

fn recipe_color(can_craft: bool) -> [f32; 4] {
    if can_craft {
        [0.45, 0.95, 0.5, 1.0]
    } else {
        [0.65, 0.68, 0.74, 1.0]
    }
}

/// Draws the title screen and keyboard hints.
///
/// # Safety
/// The OpenGL context must be current on the calling thread.
unsafe fn draw_main_menu(gl: &glow::Context, width: i32, height: i32, can_continue: bool) {
    let panel_width = width.min(360);
    let panel_height = height.min(224);
    let x = (width - panel_width) / 2;
    let y = (height - panel_height) / 2;
    let compact = panel_width < 300 || panel_height < 200;
    let title_scale = if compact { 1 } else { 3 };
    let option_scale = if compact { 1 } else { 2 };
    let panel_top = height - (y + panel_height);
    let title_y = panel_top + if compact { 12 } else { 26 };
    let first_option_y = panel_top + if compact { 34 } else { 94 };
    let option_spacing = if compact { 14 } else { 34 };
    let title = "VOXEL FRONTIER";
    draw_hud_rect(gl, x, y, panel_width, panel_height, [0.2, 0.46, 0.72, 1.0]);
    draw_hud_rect(
        gl,
        x + 3,
        y + 3,
        panel_width - 6,
        panel_height - 6,
        [0.025, 0.04, 0.055, 1.0],
    );
    draw_bitmap_text_from_top(
        gl,
        x + (panel_width - bitmap_text_width(title, title_scale)) / 2,
        title_y,
        height,
        title_scale,
        title,
        [0.55, 0.82, 1.0, 1.0],
    );
    draw_bitmap_text_from_top(
        gl,
        x + 14,
        first_option_y,
        height,
        option_scale,
        "N - NEW GAME",
        [0.95, 0.95, 0.95, 1.0],
    );
    draw_bitmap_text_from_top(
        gl,
        x + 14,
        first_option_y + option_spacing,
        height,
        option_scale,
        if can_continue {
            "C - CONTINUE"
        } else {
            "C - NO SAVE"
        },
        if can_continue {
            [0.95, 0.95, 0.95, 1.0]
        } else {
            [0.42, 0.45, 0.5, 1.0]
        },
    );
    draw_bitmap_text_from_top(
        gl,
        x + 14,
        first_option_y + option_spacing * 2,
        height,
        option_scale,
        "ESC - QUIT",
        [0.75, 0.78, 0.84, 1.0],
    );
}

/// Draws a compact victory panel with a contrasting emblem.
///
/// # Safety
/// The OpenGL context must be current on the calling thread.
unsafe fn draw_victory_overlay(gl: &glow::Context, width: i32, height: i32) {
    let panel_width = width.min(240);
    let panel_height = height.min(112);
    let x = (width - panel_width) / 2;
    let y = (height - panel_height) / 2;
    draw_hud_rect(gl, x, y, panel_width, panel_height, [0.92, 0.72, 0.22, 1.0]);
    draw_hud_rect(
        gl,
        x + 3,
        y + 3,
        panel_width - 6,
        panel_height - 6,
        [0.025, 0.04, 0.055, 1.0],
    );
    draw_hud_rect(
        gl,
        x + panel_width / 4,
        y + panel_height / 2 - 7,
        panel_width / 2,
        14,
        [0.3, 0.82, 0.4, 1.0],
    );
    draw_hud_rect(
        gl,
        x + panel_width / 2 - 7,
        y + panel_height / 4,
        14,
        panel_height / 2,
        [0.3, 0.82, 0.4, 1.0],
    );
    draw_bitmap_text(
        gl,
        x + (panel_width - bitmap_text_width("VICTORY", 3)) / 2,
        y + 20,
        3,
        "VICTORY",
        [0.95, 0.85, 0.38, 1.0],
    );
    draw_bitmap_text(
        gl,
        x + (panel_width - bitmap_text_width("R - RESTART", 2)) / 2,
        y + panel_height - 34,
        2,
        "R - RESTART",
        [0.9, 0.92, 0.96, 1.0],
    );
}

fn bitmap_glyph(character: char) -> [u8; 7] {
    match character {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [31, 4, 4, 4, 4, 4, 31],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 25, 21, 19, 19, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        '+' => [0, 4, 4, 31, 4, 4, 0],
        '/' => [1, 1, 2, 4, 8, 16, 16],
        ':' => [0, 4, 4, 0, 4, 4, 0],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        _ => [0; 7],
    }
}

fn bitmap_text_width(text: &str, scale: i32) -> i32 {
    if scale <= 0 {
        return 0;
    }
    let character_count = text.chars().count() as i32;
    if character_count == 0 {
        0
    } else {
        (character_count * 6 - 1) * scale
    }
}

/// Draws 5x7 bitmap text using horizontal runs of scissored pixels.
///
/// # Safety
/// The OpenGL context must be current on the calling thread.
unsafe fn draw_bitmap_text(
    gl: &glow::Context,
    x: i32,
    y: i32,
    scale: i32,
    text: &str,
    color: [f32; 4],
) {
    if scale <= 0 {
        return;
    }
    for (character_index, character) in text.chars().enumerate() {
        let glyph = bitmap_glyph(character.to_ascii_uppercase());
        let origin_x = x + character_index as i32 * 6 * scale;
        for (row, bits) in glyph.into_iter().enumerate() {
            let mut column = 0;
            while column < 5 {
                if bits & (1 << (4 - column)) == 0 {
                    column += 1;
                    continue;
                }
                let run_start = column;
                while column < 5 && bits & (1 << (4 - column)) != 0 {
                    column += 1;
                }
                draw_hud_rect(
                    gl,
                    origin_x + run_start * scale,
                    y + (6 - row as i32) * scale,
                    (column - run_start) * scale,
                    scale,
                    color,
                );
            }
        }
    }
}

unsafe fn draw_bitmap_text_from_top(
    gl: &glow::Context,
    x: i32,
    top_y: i32,
    viewport_height: i32,
    scale: i32,
    text: &str,
    color: [f32; 4],
) {
    draw_bitmap_text(
        gl,
        x,
        bitmap_text_y_from_top(viewport_height, top_y, scale),
        scale,
        text,
        color,
    );
}

fn bitmap_text_y_from_top(viewport_height: i32, top_y: i32, scale: i32) -> i32 {
    viewport_height - top_y - 7 * scale
}

/// Draws the inventory hotbar using scissored clears on the current OpenGL context.
///
/// # Safety
/// The OpenGL context must be current on the calling thread.
unsafe fn draw_hotbar(
    gl: &glow::Context,
    width: i32,
    height: i32,
    selected_slot: u8,
    counts: [u16; 9],
) {
    let padding = 12.min(width / 2);
    let slot_size = hotbar_slot_size(width, padding);
    if slot_size == 0 || height < slot_size + 52 {
        return;
    }

    const GAP: i32 = 4;
    const COLORS: [[f32; 3]; 9] = [
        [0.35, 0.7, 0.25],
        [0.45, 0.3, 0.15],
        [0.5, 0.5, 0.52],
        [0.65, 0.25, 0.18],
        [0.82, 0.74, 0.48],
        [0.88, 0.92, 0.98],
        [0.42, 0.25, 0.1],
        [0.18, 0.48, 0.16],
        [0.68, 0.48, 0.25],
    ];
    let total_width = slot_size * 9 + GAP * 8;
    let start_x = (width - total_width) / 2;
    let start_y = 26;
    let border = (slot_size / 12).max(2);
    let swatch_inset = (slot_size / 6).max(3);
    let pip_gap = 2;
    let pip_width = ((slot_size - 2 * swatch_inset - 4 * pip_gap) / 5).max(1);
    let pip_height = (slot_size / 12).max(2);

    for (index, color) in COLORS.iter().enumerate() {
        let x = start_x + index as i32 * (slot_size + GAP);
        let selected = selected_slot == index as u8 + 1;
        let outline = if selected {
            [1.0, 0.82, 0.28, 1.0]
        } else {
            [0.28, 0.31, 0.38, 1.0]
        };
        draw_hud_rect(gl, x, start_y, slot_size, slot_size, outline);
        draw_hud_rect(
            gl,
            x + border,
            start_y + border,
            slot_size - 2 * border,
            slot_size - 2 * border,
            [0.035, 0.045, 0.065, 1.0],
        );
        draw_hud_rect(
            gl,
            x + swatch_inset,
            start_y + swatch_inset + 2,
            slot_size - 2 * swatch_inset,
            slot_size - 2 * swatch_inset - 3,
            [color[0], color[1], color[2], 1.0],
        );

        let available = hotbar_count_pips(counts[index]);
        for pip in 0..5_i32 {
            let pip_color = if pip < i32::from(available) {
                [0.95, 0.95, 0.95, 1.0]
            } else {
                [0.12, 0.14, 0.18, 1.0]
            };
            draw_hud_rect(
                gl,
                x + swatch_inset + pip * (pip_width + pip_gap),
                start_y + 2,
                pip_width,
                pip_height,
                pip_color,
            );
        }
    }
}

fn hotbar_slot_size(width: i32, padding: i32) -> i32 {
    const SLOT_COUNT: i32 = 9;
    const GAP: i32 = 4;
    const MAX_SLOT_SIZE: i32 = 36;
    const MIN_SLOT_SIZE: i32 = 20;
    let available = width - 2 * padding - GAP * (SLOT_COUNT - 1);
    let slot_size = (available / SLOT_COUNT).min(MAX_SLOT_SIZE);
    if slot_size < MIN_SLOT_SIZE {
        0
    } else {
        slot_size
    }
}

fn hotbar_count_pips(count: u16) -> u8 {
    count.min(5) as u8
}

/// Dessine un rectangle HUD en utilisant le scissor test et un effacement couleur.
unsafe fn draw_hud_rect(
    gl: &glow::Context,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    color: [f32; 4],
) {
    if width <= 0 || height <= 0 {
        return;
    }
    gl.scissor(x, y, width, height);
    gl.clear_color(color[0], color[1], color[2], color[3]);
    gl.clear(glow::COLOR_BUFFER_BIT);
}

fn meter_fill_width(total: i32, current: u8, maximum: u8) -> i32 {
    if total <= 0 || maximum == 0 {
        return 0;
    }
    (i64::from(total) * i64::from(current.min(maximum)) / i64::from(maximum)) as i32
}

/// Envoie sommets et index au GPU dans un nouveau VAO. Le contexte OpenGL doit être courant.
fn upload_mesh(gl: &glow::Context, mesh: &MeshData) -> Result<GpuMesh, String> {
    // SAFETY: le contexte est courant sur ce thread (voir `Renderer::new`).
    unsafe {
        // Le VAO enregistre les liaisons de sommets et d'index : il doit être lié avant elles.
        let vertex_array = gl.create_vertex_array()?;
        gl.bind_vertex_array(Some(vertex_array));

        let vertex_buffer = gl.create_buffer()?;
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertex_buffer));
        gl.buffer_data_u8_slice(
            glow::ARRAY_BUFFER,
            &vertex_bytes(&mesh.vertices),
            glow::STATIC_DRAW,
        );

        let index_buffer = gl.create_buffer()?;
        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(index_buffer));
        gl.buffer_data_u8_slice(
            glow::ELEMENT_ARRAY_BUFFER,
            &u32_bytes(&mesh.indices),
            glow::STATIC_DRAW,
        );

        gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, STRIDE, 0);
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, STRIDE, COLOR_OFFSET);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(2, 3, glow::FLOAT, false, STRIDE, NORMAL_OFFSET);
        gl.enable_vertex_attrib_array(2);

        gl.bind_vertex_array(None);
        Ok(GpuMesh {
            vertex_array,
            vertex_buffer,
            index_buffer,
            index_count: mesh.indices.len() as i32,
        })
    }
}

/// Compile et lie le programme (vertex + fragment). Retourne le journal GL en cas d'échec.
///
/// # Safety
/// Le contexte OpenGL doit être courant sur le thread appelant.
unsafe fn compile_program(gl: &glow::Context) -> Result<glow::Program, String> {
    let vertex = compile_shader(gl, glow::VERTEX_SHADER, VERTEX_SHADER)?;
    let fragment = compile_shader(gl, glow::FRAGMENT_SHADER, FRAGMENT_SHADER)?;

    let program = gl.create_program()?;
    gl.attach_shader(program, vertex);
    gl.attach_shader(program, fragment);
    gl.link_program(program);

    // Les shaders restent référencés par le programme lié : on peut les supprimer dès maintenant.
    gl.delete_shader(vertex);
    gl.delete_shader(fragment);

    if !gl.get_program_link_status(program) {
        let log = gl.get_program_info_log(program);
        gl.delete_program(program);
        return Err(format!("liaison du programme impossible : {log}"));
    }
    Ok(program)
}

/// Compile un shader du type donné. Retourne le journal GL en cas d'échec.
///
/// # Safety
/// Le contexte OpenGL doit être courant sur le thread appelant.
unsafe fn compile_shader(
    gl: &glow::Context,
    kind: u32,
    source: &str,
) -> Result<glow::Shader, String> {
    let shader = gl.create_shader(kind)?;
    gl.shader_source(shader, source);
    gl.compile_shader(shader);

    if !gl.get_shader_compile_status(shader) {
        let log = gl.get_shader_info_log(shader);
        gl.delete_shader(shader);
        return Err(format!("compilation du shader impossible : {log}"));
    }
    Ok(shader)
}

/// Convertit les sommets en octets natifs : position puis couleur, flottants en ordre natif.
fn vertex_bytes(vertices: &[Vertex]) -> Vec<u8> {
    vertices
        .iter()
        .flat_map(|v| v.position.into_iter().chain(v.color).chain(v.normal))
        .flat_map(f32::to_ne_bytes)
        .collect()
}

/// Convertit les index en octets natifs.
fn u32_bytes(values: &[u32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_ne_bytes())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_jauge_de_vie_est_bornee_et_proportionnelle() {
        assert_eq!(meter_fill_width(180, 10, 10), 180);
        assert_eq!(meter_fill_width(180, 5, 10), 90);
        assert_eq!(meter_fill_width(180, 12, 10), 180);
        assert_eq!(meter_fill_width(180, 5, 0), 0);
        assert_eq!(meter_fill_width(-1, 5, 10), 0);
    }

    #[test]
    fn la_barre_rapide_s_adapte_a_la_largeur_de_fenetre() {
        assert_eq!(hotbar_slot_size(800, 12), 36);
        assert!(hotbar_slot_size(320, 12) >= 20);
        assert_eq!(hotbar_slot_size(200, 12), 0);
    }

    #[test]
    fn les_indicateurs_de_stock_sont_bornes_a_cinq() {
        assert_eq!(hotbar_count_pips(0), 0);
        assert_eq!(hotbar_count_pips(3), 3);
        assert_eq!(hotbar_count_pips(999), 5);
    }

    #[test]
    fn les_glyphes_du_menu_et_de_la_victoire_sont_definis() {
        for character in "VOXEL FRONTIER NEW GAME CONTINUE NO SAVE ESC QUIT VICTORY RESTART PAUSED RESUME CRAFTING BRICKS STONE DIRT WOOD PLANKS CLOSE OPTIONS MOUSE SENSITIVITY LEFT RIGHT PISTOL RIFLE SHOTGUN AMMO"
            .chars()
            .filter(|character| character.is_ascii_alphabetic() || character.is_ascii_digit())
        {
            assert_ne!(
                bitmap_glyph(character),
                [0; 7],
                "glyph {character} manquant"
            );
        }
        assert_ne!(bitmap_glyph('P'), [0; 7]);
        assert_eq!(bitmap_glyph(' '), [0; 7]);
        for character in "0123456789+:/".chars() {
            assert_ne!(
                bitmap_glyph(character),
                [0; 7],
                "glyph {character} manquant"
            );
        }
    }

    #[test]
    fn les_recettes_non_fabricables_sont_visuellement_distinguees() {
        assert_eq!(recipe_color(true), [0.45, 0.95, 0.5, 1.0]);
        assert_eq!(recipe_color(false), [0.65, 0.68, 0.74, 1.0]);
    }

    #[test]
    fn la_largeur_du_texte_bitmap_exclut_le_dernier_espacement() {
        assert_eq!(bitmap_text_width("", 2), 0);
        assert_eq!(bitmap_text_width("A", 2), 10);
        assert_eq!(bitmap_text_width("A B", 1), 17);
        assert_eq!(bitmap_text_width("VOXEL FRONTIER", 3), 249);
        assert_eq!(bitmap_text_width("A", 0), 0);
    }

    #[test]
    fn les_coordonnees_du_menu_sont_converties_du_haut_vers_open_gl() {
        assert_eq!(bitmap_text_y_from_top(600, 100, 1), 493);
        assert_eq!(bitmap_text_y_from_top(600, 100, 2), 486);
        assert_eq!(bitmap_text_y_from_top(600, 600, 1), -7);
    }
}
