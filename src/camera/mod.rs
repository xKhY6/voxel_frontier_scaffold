use glam::{Mat4, Vec3};
use std::f32::consts::{PI, TAU};

/// Champ de vision vertical (70 degrés).
const FOV_Y_RADIANS: f32 = 70.0 * PI / 180.0;
const NEAR: f32 = 0.1;
const FAR: f32 = 1000.0;
/// Pitch maximal (89 degrés) : évite que la direction de regard soit parallèle à l'axe Y.
const PITCH_LIMIT: f32 = 89.0 * PI / 180.0;

/// Caméra FPS : position en espace monde, yaw et pitch en radians.
#[derive(Debug, Default, Clone, Copy)]
pub struct Camera {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
}

impl Camera {
    /// Tourne la caméra de `dyaw` et `dpitch`. Le pitch est borné ; le yaw reste dans un tour.
    pub fn rotate(&mut self, dyaw: f32, dpitch: f32) {
        self.yaw = (self.yaw + dyaw).rem_euclid(TAU);
        self.pitch = (self.pitch + dpitch).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    /// Direction de regard normalisée. Yaw nul regarde vers -Z ; yaw positif tourne vers la gauche.
    pub fn forward(&self) -> Vec3 {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        Vec3::new(-sin_yaw * cos_pitch, sin_pitch, -cos_yaw * cos_pitch)
    }

    /// Matrice monde -> vue (repère droitier, Y vers le haut).
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_to_rh(self.position, self.forward(), Vec3::Y)
    }

    /// Matrice vue -> clip. Profondeur en -1..1 (convention OpenGL). `aspect` doit être > 0.
    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh_gl(FOV_Y_RADIANS, aspect, NEAR, FAR)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    const EPSILON: f32 = 1e-4;

    fn assert_proche(obtenu: Vec3, attendu: Vec3) {
        assert!(
            (obtenu - attendu).length() < EPSILON,
            "attendu {attendu:?}, obtenu {obtenu:?}"
        );
    }

    #[test]
    fn le_regard_par_defaut_est_vers_moins_z() {
        assert_proche(Camera::default().forward(), Vec3::new(0.0, 0.0, -1.0));
    }

    #[test]
    fn un_yaw_positif_tourne_a_gauche() {
        let mut camera = Camera::default();
        camera.rotate(FRAC_PI_2, 0.0);

        assert_proche(camera.forward(), Vec3::new(-1.0, 0.0, 0.0));
    }

    #[test]
    fn le_pitch_est_borne() {
        let mut camera = Camera::default();
        camera.rotate(0.0, 10.0);
        assert_eq!(camera.pitch, PITCH_LIMIT);

        camera.rotate(0.0, -20.0);
        assert_eq!(camera.pitch, -PITCH_LIMIT);
    }

    #[test]
    fn le_yaw_reste_dans_un_tour() {
        let mut camera = Camera::default();
        camera.rotate(TAU + 0.1, 0.0);

        assert!((camera.yaw - 0.1).abs() < EPSILON);
    }

    #[test]
    fn la_vue_place_la_camera_a_l_origine() {
        let camera = Camera {
            position: Vec3::new(1.0, 2.0, 3.0),
            ..Camera::default()
        };

        assert_proche(
            camera.view_matrix().transform_point3(camera.position),
            Vec3::ZERO,
        );
    }

    #[test]
    fn la_vue_fait_regarder_vers_moins_z() {
        let mut camera = Camera {
            position: Vec3::new(-4.0, 1.0, 2.0),
            ..Camera::default()
        };
        camera.rotate(0.3, 0.2);

        let direction_en_vue = camera.view_matrix().transform_vector3(camera.forward());
        assert_proche(direction_en_vue, Vec3::new(0.0, 0.0, -1.0));
    }

    #[test]
    fn la_projection_respecte_le_champ_de_vision_et_le_ratio() {
        let proj = Camera::default().projection_matrix(2.0);
        let focale = 1.0 / (FOV_Y_RADIANS / 2.0).tan();

        assert!((proj.col(1).y - focale).abs() < EPSILON);
        assert!((proj.col(0).x - focale / 2.0).abs() < EPSILON);
    }

    #[test]
    fn la_projection_envoie_near_et_far_sur_moins_1_et_1() {
        let proj = Camera::default().projection_matrix(1.0);

        let ndc_near = proj.project_point3(Vec3::new(0.0, 0.0, -NEAR)).z;
        let ndc_far = proj.project_point3(Vec3::new(0.0, 0.0, -FAR)).z;

        assert!((ndc_near + 1.0).abs() < EPSILON);
        assert!((ndc_far - 1.0).abs() < EPSILON);
    }
}
