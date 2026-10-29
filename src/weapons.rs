use serde::{Deserialize, Serialize};

const WEAPON_COUNT: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagazineSaveState {
    pub loaded: u8,
    pub reserve: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponSaveState {
    pub selected_slot: u8,
    pub magazines: [MagazineSaveState; WEAPON_COUNT],
}

impl WeaponSaveState {
    pub fn is_valid(self) -> bool {
        self.selected_slot < WEAPON_COUNT as u8
            && WeaponKind::ALL
                .iter()
                .zip(self.magazines)
                .all(|(weapon, magazine)| {
                    magazine.loaded <= weapon.magazine_size()
                        && magazine.reserve <= weapon.max_reserve()
                })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponKind {
    Pistol,
    Rifle,
    Shotgun,
}

impl WeaponKind {
    pub const ALL: [Self; WEAPON_COUNT] = [Self::Pistol, Self::Rifle, Self::Shotgun];

    pub fn name(self) -> &'static str {
        match self {
            Self::Pistol => "PISTOL",
            Self::Rifle => "RIFLE",
            Self::Shotgun => "SHOTGUN",
        }
    }

    pub fn damage(self) -> u8 {
        match self {
            Self::Pistol | Self::Rifle => 1,
            Self::Shotgun => 3,
        }
    }

    pub fn range(self) -> f32 {
        match self {
            Self::Pistol => 24.0,
            Self::Rifle => 40.0,
            Self::Shotgun => 10.0,
        }
    }

    pub fn fire_interval(self) -> f32 {
        match self {
            Self::Pistol => 0.3,
            Self::Rifle => 0.12,
            Self::Shotgun => 0.8,
        }
    }

    pub fn magazine_size(self) -> u8 {
        match self {
            Self::Pistol => 12,
            Self::Rifle => 30,
            Self::Shotgun => 5,
        }
    }

    pub fn max_reserve(self) -> u16 {
        match self {
            Self::Pistol => 60,
            Self::Rifle => 120,
            Self::Shotgun => 30,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Magazine {
    loaded: u8,
    reserve: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct WeaponInventory {
    selected: WeaponKind,
    magazines: [Magazine; WEAPON_COUNT],
    cooldown: f32,
}

impl Default for WeaponInventory {
    fn default() -> Self {
        Self {
            selected: WeaponKind::Rifle,
            magazines: [
                Magazine {
                    loaded: 12,
                    reserve: 60,
                },
                Magazine {
                    loaded: 30,
                    reserve: 120,
                },
                Magazine {
                    loaded: 5,
                    reserve: 30,
                },
            ],
            cooldown: 0.0,
        }
    }
}

impl WeaponInventory {
    pub fn selected(&self) -> WeaponKind {
        self.selected
    }

    pub fn selected_slot(&self) -> u8 {
        self.selected.index() as u8 + 1
    }

    pub fn select_next(&mut self) {
        let next = (self.selected.index() + 1) % WEAPON_COUNT;
        self.selected = WeaponKind::ALL[next];
    }

    pub fn select_slot(&mut self, slot: u8) {
        if let Some(weapon) = WeaponKind::ALL.get(slot as usize) {
            self.selected = *weapon;
        }
    }

    pub fn ammo(&self) -> (u8, u16) {
        let magazine = self.magazines[self.selected.index()];
        (magazine.loaded, magazine.reserve)
    }

    pub fn is_ready(&self) -> bool {
        self.cooldown == 0.0 && self.magazines[self.selected.index()].loaded > 0
    }

    pub fn advance(&mut self, dt: f32) {
        if dt.is_finite() && dt > 0.0 {
            self.cooldown = (self.cooldown - dt).max(0.0);
        }
    }

    pub fn fire(&mut self) -> bool {
        if self.cooldown > 0.0 {
            return false;
        }
        let magazine = &mut self.magazines[self.selected.index()];
        if magazine.loaded == 0 {
            return false;
        }
        magazine.loaded -= 1;
        self.cooldown = self.selected.fire_interval();
        true
    }

    pub fn reload(&mut self) -> u8 {
        let weapon = self.selected;
        let magazine = &mut self.magazines[weapon.index()];
        let needed = weapon.magazine_size().saturating_sub(magazine.loaded);
        let amount = needed.min(magazine.reserve.min(u16::from(u8::MAX)) as u8);
        magazine.loaded += amount;
        magazine.reserve -= u16::from(amount);
        amount
    }

    pub fn save_state(self) -> WeaponSaveState {
        WeaponSaveState {
            selected_slot: self.selected.index() as u8,
            magazines: self.magazines.map(|magazine| MagazineSaveState {
                loaded: magazine.loaded,
                reserve: magazine.reserve,
            }),
        }
    }

    pub fn from_save_state(state: WeaponSaveState) -> Option<Self> {
        if !state.is_valid() {
            return None;
        }
        Some(Self {
            selected: WeaponKind::ALL[state.selected_slot as usize],
            magazines: state.magazines.map(|magazine| Magazine {
                loaded: magazine.loaded,
                reserve: magazine.reserve,
            }),
            cooldown: 0.0,
        })
    }
}

impl WeaponKind {
    fn index(self) -> usize {
        match self {
            Self::Pistol => 0,
            Self::Rifle => 1,
            Self::Shotgun => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chaque_arme_a_un_chargeur_et_une_reserve_distincts() {
        let mut weapons = WeaponInventory::default();
        assert_eq!(weapons.selected(), WeaponKind::Rifle);
        assert_eq!(weapons.ammo(), (30, 120));

        weapons.select_slot(0);
        assert_eq!(weapons.ammo(), (12, 60));
        weapons.select_slot(2);
        assert_eq!(weapons.ammo(), (5, 30));
    }

    #[test]
    fn le_tir_consomme_une_balle_et_respecte_la_cadence() {
        let mut weapons = WeaponInventory::default();
        assert!(weapons.fire());
        assert_eq!(weapons.ammo(), (29, 120));
        assert!(!weapons.fire());

        weapons.advance(0.12);
        assert!(weapons.fire());
    }

    #[test]
    fn le_rechargement_transfere_uniquement_les_munitions_disponibles() {
        let mut weapons = WeaponInventory::default();
        weapons.select_slot(0);
        for _ in 0..10 {
            assert!(weapons.fire());
            weapons.advance(0.3);
        }
        assert_eq!(weapons.ammo(), (2, 60));
        assert_eq!(weapons.reload(), 10);
        assert_eq!(weapons.ammo(), (12, 50));
        assert_eq!(weapons.reload(), 0);
    }

    #[test]
    fn les_munitions_et_l_arme_active_sont_persistables_et_validees() {
        let mut weapons = WeaponInventory::default();
        weapons.select_slot(2);
        assert!(weapons.fire());

        let saved = weapons.save_state();
        assert_eq!(
            WeaponInventory::from_save_state(saved).unwrap().ammo(),
            (4, 30)
        );

        let mut invalid = saved;
        invalid.magazines[2].loaded = 6;
        assert!(WeaponInventory::from_save_state(invalid).is_none());
    }
}
