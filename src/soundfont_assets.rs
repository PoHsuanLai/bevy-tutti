use std::io::Cursor;
use std::sync::Arc;

use bevy_asset::{Asset, AssetLoader, LoadContext, io::Reader};
use bevy_reflect::TypePath;
use tutti::SoundFont;

/// Bevy asset wrapping a parsed SoundFont (.sf2) file.
#[derive(Asset, TypePath, Clone)]
pub struct SoundFontSource {
    soundfont: Arc<SoundFont>,
    pub preset_count: usize,
    pub instrument_count: usize,
}

impl SoundFontSource {
    pub fn from_soundfont(soundfont: Arc<SoundFont>) -> Self {
        let preset_count = soundfont.get_presets().len();
        let instrument_count = soundfont.get_instruments().len();
        Self { soundfont, preset_count, instrument_count }
    }

    pub fn soundfont(&self) -> &Arc<SoundFont> {
        &self.soundfont
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Sf2LoadError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("SF2 parse error: {0}")]
    Parse(String),
}

/// Parses SF2 binary format via RustySynth.
#[derive(Default)]
pub struct Sf2Loader;

impl AssetLoader for Sf2Loader {
    type Asset = SoundFontSource;
    type Settings = ();
    type Error = Sf2LoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let sf = SoundFont::new(&mut Cursor::new(bytes))
            .map_err(|e| Sf2LoadError::Parse(e.to_string()))?;

        Ok(SoundFontSource::from_soundfont(Arc::new(sf)))
    }

    fn extensions(&self) -> &[&str] {
        &["sf2"]
    }
}
