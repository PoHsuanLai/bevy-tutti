use bevy_ecs::prelude::*;

use crate::TuttiEngineResource;

#[derive(Component)]
pub struct ExportInProgress {
    pub(crate) handle: tutti::ExportHandle,
}

#[derive(Component)]
pub struct ExportComplete;

#[derive(Component)]
pub struct ExportFailed {
    pub error: String,
}

pub fn export_start_system(
    mut commands: Commands,
    engine: Option<Res<TuttiEngineResource>>,
    query: Query<(Entity, &crate::components::StartExport), Added<crate::components::StartExport>>,
) {
    let Some(engine) = engine else { return };

    for (entity, start) in query.iter() {
        let mut builder = engine.export();

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

        let handle = builder.start(&start.path);

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
        match export.handle.progress() {
            tutti::ExportStatus::Complete => {
                bevy_log::info!("Export complete (entity {entity:?})");
                commands
                    .entity(entity)
                    .remove::<ExportInProgress>()
                    .insert(ExportComplete);
            }
            tutti::ExportStatus::Failed(error) => {
                bevy_log::error!("Export failed (entity {entity:?}): {error}");
                commands
                    .entity(entity)
                    .remove::<ExportInProgress>()
                    .insert(ExportFailed { error });
            }
            tutti::ExportStatus::Running(_) | tutti::ExportStatus::Pending => {
            }
        }
    }
}
