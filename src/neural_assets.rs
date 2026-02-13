use bevy_asset::{Asset, AssetLoader, LoadContext, io::Reader};
use bevy_reflect::TypePath;

/// Raw model bytes. Registration is deferred to playback time because
/// it requires a running `TuttiEngineResource`.
#[derive(Asset, TypePath, Clone)]
pub struct NeuralModelSource {
    bytes: Vec<u8>,
    pub name: String,
}

impl NeuralModelSource {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Debug, thiserror::Error)]
pub enum NeuralModelLoadError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Default)]
pub struct NeuralModelLoader;

impl AssetLoader for NeuralModelLoader {
    type Asset = NeuralModelSource;
    type Settings = ();
    type Error = NeuralModelLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let name = load_context
            .path()
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(NeuralModelSource { bytes, name })
    }

    fn extensions(&self) -> &[&str] {
        &["mpk"]
    }
}
