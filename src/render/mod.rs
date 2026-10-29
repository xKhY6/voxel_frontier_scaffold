//! Rendu OpenGL 3.3 : un maillage GPU par chunk, dessiné avec la matrice projection * vue.

mod context;

use std::collections::HashMap;

use glam::{IVec3, Mat4};
use glow::HasContext;

pub use context::OpenGlWindow;

use crate::voxel::mesh::{MeshData, Vertex};

const VERTEX_SHADER: &str = include_str!("../../assets/shaders/cube.vert");
const FRAGMENT_SHADER: &str = include_str!("../../assets/shaders/cube.frag");

/// Pas entre deux sommets, en octets. `Vertex` est `repr(C)` : pas de remplissage.
const STRIDE: i32 = std::mem::size_of::<Vertex>() as i32;

/// Décalage de la couleur dans un sommet, en octets (après la position).
const COLOR_OFFSET: i32 = std::mem::size_of::<[f32; 3]>() as i32;

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
    meshes: HashMap<IVec3, GpuMesh>,
}

impl Renderer {
    /// Compile les shaders et prépare l'état GL. Le contexte OpenGL doit être courant.
    pub fn new(gl: &glow::Context) -> Result<Self, String> {
        // SAFETY: le contexte est courant sur ce thread (garanti par `OpenGlWindow::new`).
        unsafe {
            let program = compile_program(gl)?;
            let view_proj_location = gl.get_uniform_location(program, "u_view_proj");

            // Sans test de profondeur, les faces arrière se dessineraient par-dessus les avant.
            gl.enable(glow::DEPTH_TEST);
            // Seules les faces antihoraires vues de l'extérieur sont dessinées (voir `voxel::mesh`).
            gl.enable(glow::CULL_FACE);
            gl.clear_color(0.05, 0.05, 0.08, 1.0);

            Ok(Self {
                program,
                view_proj_location,
                meshes: HashMap::new(),
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
    pub fn draw(&self, gl: &glow::Context, width: u32, height: u32, view_proj: Mat4) {
        // SAFETY: le contexte est courant sur ce thread (voir `Renderer::new`).
        unsafe {
            gl.viewport(0, 0, width as i32, height as i32);
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);

            gl.use_program(Some(self.program));
            gl.uniform_matrix_4_f32_slice(
                self.view_proj_location.as_ref(),
                false,
                &view_proj.to_cols_array(),
            );

            for mesh in self.meshes.values() {
                gl.bind_vertex_array(Some(mesh.vertex_array));
                gl.draw_elements(glow::TRIANGLES, mesh.index_count, glow::UNSIGNED_INT, 0);
            }
        }
    }
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
        .flat_map(|v| v.position.into_iter().chain(v.color))
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
