use bevy_ecs::prelude::*;

use crate::components::{AddLfo, AudioEmitter};
use crate::TuttiEngineResource;

#[cfg(feature = "dsp")]
use crate::components::{AddCompressor, AddGate};

#[cfg(feature = "dsp")]
pub fn dsp_compressor_system(
    mut commands: Commands,
    engine: Option<Res<TuttiEngineResource>>,
    query: Query<(Entity, &AddCompressor), Added<AddCompressor>>,
) {
    let Some(engine) = engine else { return };

    for (entity, add) in query.iter() {
        let node_id = engine.graph_mut(|net| {
            if add.stereo {
                let comp = tutti::StereoSidechainCompressor::new(
                    add.threshold_db,
                    add.ratio,
                    add.attack,
                    add.release,
                )
                .with_makeup(add.makeup_db);
                net.add(comp).id()
            } else {
                let comp = tutti::SidechainCompressor::builder()
                    .threshold_db(add.threshold_db)
                    .ratio(add.ratio)
                    .attack_seconds(add.attack)
                    .release_seconds(add.release)
                    .makeup_gain_db(add.makeup_db)
                    .build();
                net.add(comp).id()
            }
        });

        commands
            .entity(entity)
            .remove::<AddCompressor>()
            .insert(AudioEmitter { node_id });

        bevy_log::info!(
            "Compressor added (entity {entity:?}, stereo={}, node {node_id:?})",
            add.stereo
        );
    }
}

#[cfg(feature = "dsp")]
pub fn dsp_gate_system(
    mut commands: Commands,
    engine: Option<Res<TuttiEngineResource>>,
    query: Query<(Entity, &AddGate), Added<AddGate>>,
) {
    let Some(engine) = engine else { return };

    for (entity, add) in query.iter() {
        let node_id = engine.graph_mut(|net| {
            if add.stereo {
                let gate = tutti::StereoSidechainGate::new(
                    add.threshold_db,
                    add.attack,
                    add.hold,
                    add.release,
                );
                net.add(gate).id()
            } else {
                let gate = tutti::SidechainGate::builder()
                    .threshold_db(add.threshold_db)
                    .attack_seconds(add.attack)
                    .hold_seconds(add.hold)
                    .release_seconds(add.release)
                    .build();
                net.add(gate).id()
            }
        });

        commands
            .entity(entity)
            .remove::<AddGate>()
            .insert(AudioEmitter { node_id });

        bevy_log::info!(
            "Gate added (entity {entity:?}, stereo={}, node {node_id:?})",
            add.stereo
        );
    }
}

pub fn dsp_lfo_system(
    mut commands: Commands,
    engine: Option<Res<TuttiEngineResource>>,
    query: Query<(Entity, &AddLfo), Added<AddLfo>>,
) {
    let Some(engine) = engine else { return };

    for (entity, add) in query.iter() {
        let node_id = if add.beat_synced {
            let transport = engine.transport();
            let lfo = tutti::LfoNode::with_transport(add.shape, add.frequency, transport);
            lfo.set_depth(add.depth);
            engine.graph_mut(|net| net.add(lfo).id())
        } else {
            let lfo = tutti::LfoNode::new(add.shape, add.frequency);
            lfo.set_depth(add.depth);
            engine.graph_mut(|net| net.add(lfo).id())
        };

        commands
            .entity(entity)
            .remove::<AddLfo>()
            .insert(AudioEmitter { node_id });

        bevy_log::info!(
            "LFO added (entity {entity:?}, beat_synced={}, node {node_id:?})",
            add.beat_synced
        );
    }
}
