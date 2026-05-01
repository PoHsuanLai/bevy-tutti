//! Spawn systems for DSP units.
//!
//! Each `Add*` marker, when added to an entity, is consumed by its
//! sibling system here, which builds the corresponding tutti unit, adds
//! it to the graph, and inserts the modern entity-as-node shape:
//!
//! ```text
//! AudioNode(id)        — wraps the tutti NodeId
//! NodeKind::*          — dispatch tag for parameter reconcilers
//! Volume / Pan / Mute  — channel-strip-style level (where applicable)
//! <typed param components> — Frequency / Q / DelayTime / …
//! ```
//!
//! The legacy `AudioEmitter` component is *not* attached by these
//! systems anymore — `AudioNode + NodeKind` is the canonical shape and
//! every reconciler in [`crate::graph_reconcile`] dispatches off it.
//! Older code paths (sampler / plugin / midi) still attach
//! `AudioEmitter` for backwards compat; those migrate as their
//! lifecycles are cleaned up.

use bevy_ecs::prelude::*;

use tutti::core::ecs::{
    Attack, AudioNode, CompressorRatio, DelayTime, Feedback, FilterQ, Frequency, GainDb, ModDepth,
    ModRate, Mute, NodeKind, Pan, Release, ReverbDamping, ReverbRoomSize, ThresholdDb, Volume,
    WetMix,
};

use crate::graph_reconcile::GraphDirty;
use crate::{TransportRes, TuttiGraphRes};

#[cfg(feature = "dsp")]
use crate::components::{
    AddChorus, AddCompressor, AddDelay, AddFilter, AddGate, AddLfo, AddReverb,
};

#[cfg(not(feature = "dsp"))]
use crate::components::AddLfo;

// =============================================================================
// Compressor
// =============================================================================

#[cfg(feature = "dsp")]
pub fn dsp_compressor_system(
    mut commands: Commands,
    graph: Option<ResMut<TuttiGraphRes>>,
    mut dirty: ResMut<GraphDirty>,
    query: Query<(Entity, &AddCompressor), Added<AddCompressor>>,
) {
    let Some(mut graph) = graph else { return };

    for (entity, add) in query.iter() {
        let comp = if add.stereo {
            tutti::units::Compressor::stereo(add.threshold_db, add.ratio, add.attack, add.release)
        } else {
            tutti::units::Compressor::mono(add.threshold_db, add.ratio, add.attack, add.release)
        }
        .with_makeup(add.makeup_db);
        let node_id = graph.0.add(comp);
        dirty.0 = true;

        commands
            .entity(entity)
            .remove::<AddCompressor>()
            .insert((
                AudioNode(node_id),
                NodeKind::Compressor,
                ThresholdDb(add.threshold_db),
                CompressorRatio(add.ratio),
                Attack(add.attack),
                Release(add.release),
                GainDb(add.makeup_db),
            ));

        bevy_log::info!(
            "Compressor added (entity {entity:?}, stereo={}, node {node_id:?})",
            add.stereo
        );
    }
}

// =============================================================================
// Gate
// =============================================================================

#[cfg(feature = "dsp")]
pub fn dsp_gate_system(
    mut commands: Commands,
    graph: Option<ResMut<TuttiGraphRes>>,
    mut dirty: ResMut<GraphDirty>,
    query: Query<(Entity, &AddGate), Added<AddGate>>,
) {
    let Some(mut graph) = graph else { return };

    for (entity, add) in query.iter() {
        let gate = if add.stereo {
            tutti::units::Gate::stereo(add.threshold_db, add.attack, add.hold, add.release)
        } else {
            tutti::units::Gate::mono(add.threshold_db, add.attack, add.hold, add.release)
        };
        let node_id = graph.0.add(gate);
        dirty.0 = true;

        commands
            .entity(entity)
            .remove::<AddGate>()
            .insert((
                AudioNode(node_id),
                NodeKind::Gate,
                ThresholdDb(add.threshold_db),
                Attack(add.attack),
                Release(add.release),
            ));

        bevy_log::info!(
            "Gate added (entity {entity:?}, stereo={}, node {node_id:?})",
            add.stereo
        );
    }
}

// =============================================================================
// LFO
// =============================================================================

pub fn dsp_lfo_system(
    mut commands: Commands,
    graph: Option<ResMut<TuttiGraphRes>>,
    transport: Option<Res<TransportRes>>,
    mut dirty: ResMut<GraphDirty>,
    query: Query<(Entity, &AddLfo), Added<AddLfo>>,
) {
    let Some(mut graph) = graph else { return };

    for (entity, add) in query.iter() {
        let node_id = if add.beat_synced {
            let Some(transport) = transport.as_ref() else {
                bevy_log::warn!("Beat-synced LFO requested but no TransportRes available");
                continue;
            };
            let lfo = tutti::units::LfoNode::new(add.shape)
                .with_beat_sync(transport.0.clone(), add.frequency);
            lfo.set_depth(add.depth);
            graph.0.add(lfo)
        } else {
            let lfo = tutti::units::LfoNode::new(add.shape).with_frequency(add.frequency);
            lfo.set_depth(add.depth);
            graph.0.add(lfo)
        };
        dirty.0 = true;

        commands
            .entity(entity)
            .remove::<AddLfo>()
            .insert((
                AudioNode(node_id),
                NodeKind::Lfo,
                Frequency(add.frequency),
                ModDepth(add.depth),
            ));

        bevy_log::info!(
            "LFO added (entity {entity:?}, beat_synced={}, node {node_id:?})",
            add.beat_synced
        );
    }
}

// =============================================================================
// Filter (StereoSvfFilterNode)
// =============================================================================

#[cfg(feature = "dsp")]
pub fn dsp_filter_system(
    mut commands: Commands,
    graph: Option<ResMut<TuttiGraphRes>>,
    mut dirty: ResMut<GraphDirty>,
    query: Query<(Entity, &AddFilter), Added<AddFilter>>,
) {
    let Some(mut graph) = graph else { return };

    for (entity, add) in query.iter() {
        let mut node = tutti::units::StereoSvfFilterNode::<f64>::new(
            add.svf_type,
            add.frequency,
            add.q,
        );
        if add.gain_db != 0.0 {
            node = node.with_gain_db(add.gain_db);
        }
        let node_id = graph.0.add(node);
        dirty.0 = true;

        commands
            .entity(entity)
            .remove::<AddFilter>()
            .insert((
                AudioNode(node_id),
                NodeKind::Filter,
                Frequency(add.frequency),
                FilterQ(add.q),
                GainDb(add.gain_db),
            ));

        bevy_log::info!(
            "Filter added (entity {entity:?}, type={:?}, node {node_id:?})",
            add.svf_type
        );
    }
}

// =============================================================================
// Reverb (fundsp reverb_stereo)
// =============================================================================

#[cfg(feature = "dsp")]
pub fn dsp_reverb_system(
    mut commands: Commands,
    graph: Option<ResMut<TuttiGraphRes>>,
    mut dirty: ResMut<GraphDirty>,
    query: Query<(Entity, &AddReverb), Added<AddReverb>>,
) {
    let Some(mut graph) = graph else { return };

    for (entity, add) in query.iter() {
        let reverb = tutti::dsp::reverb_stereo(
            add.room_size as f64,
            add.time_secs as f64,
            add.damping as f64,
        );
        let node_id = graph.0.add(reverb);
        dirty.0 = true;

        commands
            .entity(entity)
            .remove::<AddReverb>()
            .insert((
                AudioNode(node_id),
                NodeKind::Reverb,
                ReverbRoomSize(add.room_size),
                ReverbDamping(add.damping),
                WetMix(add.wet),
            ));

        bevy_log::info!("Reverb added (entity {entity:?}, node {node_id:?})");
    }
}

// =============================================================================
// Delay (StereoDelayLineNode)
// =============================================================================

#[cfg(feature = "dsp")]
pub fn dsp_delay_system(
    mut commands: Commands,
    graph: Option<ResMut<TuttiGraphRes>>,
    mut dirty: ResMut<GraphDirty>,
    query: Query<(Entity, &AddDelay), Added<AddDelay>>,
) {
    let Some(mut graph) = graph else { return };

    for (entity, add) in query.iter() {
        let delay = tutti::units::StereoDelayLineNode::new(
            add.max_delay_secs,
            add.delay_time_secs,
            add.delay_time_secs,
            add.feedback,
        );
        delay.set_mix(add.wet);
        let node_id = graph.0.add(delay);
        dirty.0 = true;

        commands
            .entity(entity)
            .remove::<AddDelay>()
            .insert((
                AudioNode(node_id),
                NodeKind::Delay,
                DelayTime(add.delay_time_secs),
                Feedback(add.feedback),
                WetMix(add.wet),
            ));

        bevy_log::info!("Delay added (entity {entity:?}, node {node_id:?})");
    }
}

// =============================================================================
// Chorus (ChorusNode)
// =============================================================================

#[cfg(feature = "dsp")]
pub fn dsp_chorus_system(
    mut commands: Commands,
    graph: Option<ResMut<TuttiGraphRes>>,
    mut dirty: ResMut<GraphDirty>,
    query: Query<(Entity, &AddChorus), Added<AddChorus>>,
) {
    let Some(mut graph) = graph else { return };

    for (entity, add) in query.iter() {
        let chorus = tutti::units::ChorusNode::new();
        chorus.set_rate(add.rate_hz);
        chorus.set_depth(add.depth_secs);
        chorus.set_feedback(add.feedback);
        chorus.set_mix(add.wet);
        let node_id = graph.0.add(chorus);
        dirty.0 = true;

        commands
            .entity(entity)
            .remove::<AddChorus>()
            .insert((
                AudioNode(node_id),
                NodeKind::Chorus,
                ModRate(add.rate_hz),
                ModDepth(add.depth_secs),
                Feedback(add.feedback),
                WetMix(add.wet),
            ));

        bevy_log::info!("Chorus added (entity {entity:?}, node {node_id:?})");
    }
}

// Suppress unused-import warning when the dsp feature is off.
#[cfg(not(feature = "dsp"))]
fn _unused_imports(
    _: AudioNode,
    _: NodeKind,
    _: Volume,
    _: Pan,
    _: Mute,
    _: ThresholdDb,
    _: CompressorRatio,
    _: Attack,
    _: Release,
    _: GainDb,
    _: Frequency,
    _: FilterQ,
    _: ReverbRoomSize,
    _: ReverbDamping,
    _: WetMix,
    _: DelayTime,
    _: Feedback,
    _: ModRate,
    _: ModDepth,
) {
}
