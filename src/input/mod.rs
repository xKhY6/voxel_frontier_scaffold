use std::collections::HashSet;

use winit::event::{ElementState, MouseButton};
use winit::keyboard::KeyCode;

/// État des entrées clavier et souris, alimenté par les événements winit.
#[derive(Debug, Default)]
pub struct InputState {
    keys_down: HashSet<KeyCode>,
    buttons_down: HashSet<MouseButton>,
    // Déplacement brut accumulé depuis la dernière lecture, consommé par `take_mouse_delta`.
    mouse_delta: (f64, f64),
}

impl InputState {
    /// Enregistre un appui ou un relâchement. Un appui répété (auto-repeat) reste sans effet.
    pub fn set_key(&mut self, key: KeyCode, state: ElementState) {
        match state {
            ElementState::Pressed => self.keys_down.insert(key),
            ElementState::Released => self.keys_down.remove(&key),
        };
    }

    pub fn is_key_down(&self, key: KeyCode) -> bool {
        self.keys_down.contains(&key)
    }

    /// Enregistre un bouton de souris pressé ou relâché.
    pub fn set_mouse_button(&mut self, button: MouseButton, state: ElementState) {
        match state {
            ElementState::Pressed => self.buttons_down.insert(button),
            ElementState::Released => self.buttons_down.remove(&button),
        };
    }

    #[allow(dead_code)] // Lecture des boutons, pas encore utilisée par le jeu.
    pub fn is_mouse_button_down(&self, button: MouseButton) -> bool {
        self.buttons_down.contains(&button)
    }

    /// Ajoute un déplacement brut de souris. Plusieurs événements dans une même image s'additionnent.
    pub fn add_mouse_motion(&mut self, dx: f64, dy: f64) {
        self.mouse_delta.0 += dx;
        self.mouse_delta.1 += dy;
    }

    /// Retourne le déplacement accumulé puis le remet à zéro. À appeler une fois par pas de simulation.
    pub fn take_mouse_delta(&mut self) -> (f64, f64) {
        std::mem::take(&mut self.mouse_delta)
    }

    /// Oublie toutes les touches et boutons. À appeler quand la fenêtre perd le focus, car les relâchements ne seraient pas reçus.
    pub fn clear(&mut self) {
        self.keys_down.clear();
        self.buttons_down.clear();
        self.mouse_delta = (0.0, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appui_puis_relachement_d_une_touche() {
        let mut input = InputState::default();

        input.set_key(KeyCode::KeyW, ElementState::Pressed);
        assert!(input.is_key_down(KeyCode::KeyW));

        input.set_key(KeyCode::KeyW, ElementState::Released);
        assert!(!input.is_key_down(KeyCode::KeyW));
    }

    #[test]
    fn un_appui_repete_ne_change_rien() {
        let mut input = InputState::default();

        input.set_key(KeyCode::KeyA, ElementState::Pressed);
        input.set_key(KeyCode::KeyA, ElementState::Pressed);
        input.set_key(KeyCode::KeyA, ElementState::Released);

        assert!(!input.is_key_down(KeyCode::KeyA));
    }

    #[test]
    fn bouton_de_souris() {
        let mut input = InputState::default();

        input.set_mouse_button(MouseButton::Left, ElementState::Pressed);
        assert!(input.is_mouse_button_down(MouseButton::Left));
        assert!(!input.is_mouse_button_down(MouseButton::Right));

        input.set_mouse_button(MouseButton::Left, ElementState::Released);
        assert!(!input.is_mouse_button_down(MouseButton::Left));
    }

    #[test]
    fn le_deplacement_s_accumule_puis_se_remet_a_zero() {
        let mut input = InputState::default();

        input.add_mouse_motion(3.0, -1.0);
        input.add_mouse_motion(2.0, 4.0);

        assert_eq!(input.take_mouse_delta(), (5.0, 3.0));
        assert_eq!(input.take_mouse_delta(), (0.0, 0.0));
    }

    #[test]
    fn clear_relache_toutes_les_entrees() {
        let mut input = InputState::default();
        input.set_key(KeyCode::Space, ElementState::Pressed);
        input.set_mouse_button(MouseButton::Right, ElementState::Pressed);
        input.add_mouse_motion(4.0, 2.0);

        input.clear();

        assert!(!input.is_key_down(KeyCode::Space));
        assert!(!input.is_mouse_button_down(MouseButton::Right));
        assert_eq!(input.take_mouse_delta(), (0.0, 0.0));
    }
}
