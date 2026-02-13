use std::sync::Arc;

use bevy_asset::{Asset, AssetLoader, LoadContext, io::Reader};
use bevy_reflect::TypePath;
use tutti::Wave;

/// Bevy asset wrapping tutti's `Wave` type.
#[derive(Asset, TypePath, Clone)]
pub struct TuttiAudioSource {
    wave: Arc<Wave>,
    pub duration_seconds: f64,
    pub sample_rate: f64,
    pub channels: usize,
}

impl TuttiAudioSource {
    pub fn from_wave(wave: Arc<Wave>) -> Self {
        let duration_seconds = wave.duration();
        let sample_rate = wave.sample_rate();
        let channels = wave.channels();
        Self {
            wave,
            duration_seconds,
            sample_rate,
            channels,
        }
    }

    pub fn wave(&self) -> &Arc<Wave> {
        &self.wave
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AudioLoadError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Audio decode error: {0}")]
    Decode(String),
}

/// Decodes WAV, FLAC, MP3, OGG via tutti's Symphonia backend.
#[derive(Default)]
pub struct TuttiAudioLoader;

impl AssetLoader for TuttiAudioLoader {
    type Asset = TuttiAudioSource;
    type Settings = ();
    type Error = AudioLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let wave = Wave::load_slice(bytes)
            .map_err(|e| AudioLoadError::Decode(e.to_string()))?;

        Ok(TuttiAudioSource::from_wave(Arc::new(wave)))
    }

    fn extensions(&self) -> &[&str] {
        &["wav", "flac", "mp3", "ogg"]
    }
}
