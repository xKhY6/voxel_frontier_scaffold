//! Fenêtre winit avec contexte OpenGL 3.3 core, créé et rendu courant via glutin.

use std::error::Error;

use glutin::config::ConfigTemplateBuilder;
use glutin::context::{
    ContextApi, ContextAttributesBuilder, GlProfile, NotCurrentGlContext, PossiblyCurrentContext,
    Version,
};
use glutin::display::{GetGlDisplay, GlDisplay};
use glutin::surface::{GlSurface, Surface, SurfaceAttributesBuilder, WindowSurface};
use glutin_winit::{DisplayBuilder, GlWindow};
use raw_window_handle::HasWindowHandle;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes};

/// Fenêtre avec sa surface et son contexte OpenGL, prêts à dessiner sur le thread courant.
/// L'ordre des champs compte : la fenêtre est déclarée en dernier pour être détruite après la surface.
pub struct OpenGlWindow {
    gl: glow::Context,
    surface: Surface<WindowSurface>,
    context: PossiblyCurrentContext,
    window: Window,
}

impl OpenGlWindow {
    /// Crée la fenêtre et un contexte OpenGL 3.3 core, puis le rend courant sur ce thread.
    pub fn new(
        event_loop: &ActiveEventLoop,
        attributes: WindowAttributes,
    ) -> Result<Self, Box<dyn Error>> {
        let (window, gl_config) = DisplayBuilder::new()
            .with_window_attributes(Some(attributes))
            .build(
                event_loop,
                ConfigTemplateBuilder::new().with_depth_size(24),
                |mut configs| {
                    // glutin fournit au moins une configuration quand le modèle est accepté.
                    configs
                        .next()
                        .expect("aucune configuration OpenGL disponible")
                },
            )?;
        let window = window.ok_or("fenêtre non créée par glutin")?;

        let display = gl_config.display();
        let raw_handle = window.window_handle()?.as_raw();
        let context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
            .with_profile(GlProfile::Core)
            .build(Some(raw_handle));

        // SAFETY: la fenêtre est stockée avec le contexte et reste vivante tant qu'il existe.
        let not_current = unsafe { display.create_context(&gl_config, &context_attributes)? };

        let surface_attributes =
            window.build_surface_attributes(SurfaceAttributesBuilder::default())?;
        // SAFETY: la surface est liée à la fenêtre, qui vit aussi longtemps qu'elle.
        let surface = unsafe { display.create_window_surface(&gl_config, &surface_attributes)? };

        let context = not_current.make_current(&surface)?;

        // SAFETY: le contexte vient d'être rendu courant sur ce thread, requis pour charger les fonctions GL.
        let gl = unsafe {
            glow::Context::from_loader_function_cstr(|name| display.get_proc_address(name))
        };

        Ok(Self {
            gl,
            surface,
            context,
            window,
        })
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn gl(&self) -> &glow::Context {
        &self.gl
    }

    /// Adapte la surface à la taille de la fenêtre ; sans effet si une dimension est nulle.
    pub fn resize(&self) {
        self.window.resize_surface(&self.surface, &self.context);
    }

    /// Présente l'image dessinée à l'écran.
    pub fn swap_buffers(&self) -> Result<(), glutin::error::Error> {
        self.surface.swap_buffers(&self.context)
    }
}
