use std::time::Duration;

use rodio::source::{SineWave, Source};
use rodio::{DeviceSinkBuilder, MixerDeviceSink};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundEffect {
    Mine,
    Place,
    Craft,
    Hit,
    Hurt,
    Victory,
}

impl SoundEffect {
    fn tone(self) -> (f32, Duration, f32) {
        match self {
            Self::Mine => (240.0, Duration::from_millis(55), 0.12),
            Self::Place => (330.0, Duration::from_millis(65), 0.12),
            Self::Craft => (520.0, Duration::from_millis(120), 0.14),
            Self::Hit => (440.0, Duration::from_millis(75), 0.12),
            Self::Hurt => (150.0, Duration::from_millis(140), 0.18),
            Self::Victory => (740.0, Duration::from_millis(240), 0.16),
        }
    }
}

pub struct AudioPlayer {
    sink: MixerDeviceSink,
}

impl AudioPlayer {
    pub fn new() -> Result<Self, String> {
        let mut sink = DeviceSinkBuilder::open_default_sink().map_err(|error| error.to_string())?;
        sink.log_on_drop(false);
        Ok(Self { sink })
    }

    pub fn play(&self, effect: SoundEffect) {
        let (frequency, duration, volume) = effect.tone();
        self.sink.mixer().add(
            SineWave::new(frequency)
                .take_duration(duration)
                .amplify(volume),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chaque_effet_a_un_timbre_et_une_duree_valides() {
        for effect in [
            SoundEffect::Mine,
            SoundEffect::Place,
            SoundEffect::Craft,
            SoundEffect::Hit,
            SoundEffect::Hurt,
            SoundEffect::Victory,
        ] {
            let (frequency, duration, volume) = effect.tone();
            assert!(frequency.is_finite() && frequency > 0.0);
            assert!(!duration.is_zero());
            assert!((0.0..=1.0).contains(&volume));
        }
    }
}
