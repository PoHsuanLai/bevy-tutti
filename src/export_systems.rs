use bevy_ecs::prelude::*;

use crate::{AudioConfig, TuttiGraphRes};

#[derive(Component)]
pub struct ExportInProgress {
    pub(crate) handle: tutti::export::Handle<tutti::export::Written>,
}

#[derive(Component)]
pub struct ExportComplete;

#[derive(Component)]
pub struct ExportFailed {
    pub error: String,
}

pub fn export_start_system(
    mut commands: Commands,
    graph: Option<Res<TuttiGraphRes>>,
    config: Option<Res<AudioConfig>>,
    query: Query<(Entity, &crate::components::StartExport), Added<crate::components::StartExport>>,
) {
    let Some(graph) = graph else { return };
    let Some(config) = config else { return };

    for (entity, start) in query.iter() {
        let net = graph.0.clone_net();
        let mut builder = tutti::export::Export::graph(net, config.sample_rate);

        if let Some(seconds) = start.duration_seconds {
            builder = builder.duration_seconds(seconds);
        }
        if let Some((beats, tempo)) = start.duration_beats {
            builder = builder.duration_beats(beats, tempo);
        }
        if let Some(format) = start.format {
            builder = builder.format(format);
        }
        if let Some(normalization) = start.normalization {
            builder = builder.normalize(normalization);
        }

        let handle = builder.to_file(&start.path).spawn();

        bevy_log::info!("Export started: {}", start.path.display());

        commands
            .entity(entity)
            .remove::<crate::components::StartExport>()
            .insert(ExportInProgress { handle });
    }
}

pub fn export_poll_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut ExportInProgress)>,
) {
    for (entity, mut export) in query.iter_mut() {
        match export.handle.poll() {
            tutti::export::State::Done(_written) => {
                bevy_log::info!("Export complete (entity {entity:?})");
                commands
                    .entity(entity)
                    .remove::<ExportInProgress>()
                    .insert(ExportComplete);
            }
            tutti::export::State::Failed(error) => {
                bevy_log::error!("Export failed (entity {entity:?}): {error}");
                commands
                    .entity(entity)
                    .remove::<ExportInProgress>()
                    .insert(ExportFailed {
                        error: error.to_string(),
                    });
            }
            tutti::export::State::Running { .. } | tutti::export::State::Pending => {}
        }
    }
}
