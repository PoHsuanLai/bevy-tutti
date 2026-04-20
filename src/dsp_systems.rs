use bevy_ecs::prelude::*;

use crate::components::{AddLfo, AudioEmitter};
use crate::{TransportRes, TuttiGraphRes};

#[cfg(feature = "dsp")]
use crate::components::{AddCompressor, AddGate};

#[cfg(feature = "dsp")]
pub fn dsp_compressor_system(
    mut commands: Commands,
    graph: Option<ResMut<TuttiGraphRes>>,
    query: Query<(Entity, &AddCompressor), Added<AddCompressor>>,
) {
    let Some(mut graph) = graph else { return };

    let mut edited = false;
    for (entity, add) in query.iter() {
        let comp = if add.stereo {
            tutti::units::Compressor::stereo(add.threshold_db, add.ratio, add.attack, add.release)
        } else {
            tutti::units::Compressor::mono(add.threshold_db, add.ratio, add.attack, add.release)
        }
        .with_makeup(add.makeup_db);
        let node_id = graph.0.add(Box::new(comp));
        edited = true;

        commands
            .entity(entity)
            .remove::<AddCompressor>()
            .insert(AudioEmitter { node_id });

        bevy_log::info!(
            "Compressor added (entity {entity:?}, stereo={}, node {node_id:?})",
            add.stereo
        );
    }

    if edited {
        graph.0.commit();
    }
}

#[cfg(feature = "dsp")]
pub fn dsp_gate_system(
    mut commands: Commands,
    graph: Option<ResMut<TuttiGraphRes>>,
    query: Query<(Entity, &AddGate), Added<AddGate>>,
) {
    let Some(mut graph) = graph else { return };

    let mut edited = false;
    for (entity, add) in query.iter() {
        let gate = if add.stereo {
            tutti::units::Gate::stereo(add.threshold_db, add.attack, add.hold, add.release)
        } else {
            tutti::units::Gate::mono(add.threshold_db, add.attack, add.hold, add.release)
        };
        let node_id = graph.0.add(Box::new(gate));
        edited = true;

        commands
            .entity(entity)
            .remove::<AddGate>()
            .insert(AudioEmitter { node_id });

        bevy_log::info!(
            "Gate added (entity {entity:?}, stereo={}, node {node_id:?})",
            add.stereo
        );
    }

    if edited {
        graph.0.commit();
    }
}

pub fn dsp_lfo_system(
    mut commands: Commands,
    graph: Option<ResMut<TuttiGraphRes>>,
    transport: Option<Res<TransportRes>>,
    query: Query<(Entity, &AddLfo), Added<AddLfo>>,
) {
    let Some(mut graph) = graph else { return };

    let mut edited = false;
    for (entity, add) in query.iter() {
        let node_id = if add.beat_synced {
            let Some(transport) = transport.as_ref() else {
                bevy_log::warn!("Beat-synced LFO requested but no TransportRes available");
                continue;
            };
            let lfo = tutti::units::LfoNode::with_transport(
                add.shape,
                add.frequency,
                transport.0.clone(),
            );
            lfo.set_depth(add.depth);
            graph.0.add(Box::new(lfo))
        } else {
            let lfo = tutti::units::LfoNode::new(add.shape, add.frequency);
            lfo.set_depth(add.depth);
            graph.0.add(Box::new(lfo))
        };
        edited = true;

        commands
            .entity(entity)
            .remove::<AddLfo>()
            .insert(AudioEmitter { node_id });

        bevy_log::info!(
            "LFO added (entity {entity:?}, beat_synced={}, node {node_id:?})",
            add.beat_synced
        );
    }

    if edited {
        graph.0.commit();
    }
}
