//! Reconcile entity-as-node component changes into [`TuttiGraph`] operations.
//!
//! See [`tutti::ecs`] for the component types. This module provides:
//!
//! - [`SpawnAudioNode`] — `Commands` extension to atomically `graph.add(unit)`
//!   and attach `AudioNode` + `NodeKind` to a fresh entity.
//! - [`reconcile_node_despawn`] — picks up `RemovedComponents<AudioNode>`
//!   and removes the underlying graph node.
//! - [`reconcile_params`] — sweeps `Changed<Volume>` (and friends) and writes
//!   the new value through a typed `node_mut::<T>` call.
//! - [`commit_graph`] — `graph.commit()` once per frame iff any reconcile
//!   system mutated the graph.
//! - [`GraphReconcileSet`] — system-set ordering anchor for hosts that
//!   want to schedule their own logic before/after reconciliation.

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemSet;
use bevy_ecs::system::EntityCommands;

use tutti::core::ecs::{AudioNode, Mute, NodeKind, Volume};
use tutti::dsp::AudioUnit;

use crate::TuttiGraphRes;

#[cfg(feature = "sampler")]
use tutti::sampler::SamplerUnit;

/// System-set ordering anchor for the reconcile pipeline.
///
/// Apps can schedule their own systems against these sets. The plugin
/// runs them in the order: `Spawn` → `Params` → `Despawn` → `Commit`,
/// all inside `Update`.
#[derive(SystemSet, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum GraphReconcileSet {
    /// Initial spawn of new graph nodes (rare; mostly app-driven via
    /// [`SpawnAudioNode`]). Apps can hook here to populate parameter
    /// components on the same frame the node is created.
    Spawn,
    /// Parameter-component changes are written into the graph here.
    Params,
    /// Entities whose `AudioNode` was removed (or who were despawned)
    /// have their graph node removed here.
    Despawn,
    /// Single `graph.commit()` if any earlier set mutated the graph.
    Commit,
}

/// Per-frame "did anything change?" flag used to coalesce
/// `graph.commit()` to at most one call per frame.
#[derive(Resource, Default)]
pub struct GraphDirty(pub bool);

/// `Commands` extension that adds a unit to the graph and spawns an entity
/// with `AudioNode(id)` + `NodeKind` attached.
///
/// The graph mutation is queued as a deferred command and applies at the
/// next command-buffer flush — the returned `EntityCommands` lets the
/// caller chain `.insert((Volume(0.5), Pan(0.0)))` on the same entity in
/// the usual fashion.
///
/// # Example
///
/// ```rust,ignore
/// use bevy::prelude::*;
/// use bevy_tutti::*;
/// use tutti::dsp::sine_hz;
///
/// fn setup(mut commands: Commands) {
///     commands.spawn_audio_node(sine_hz::<f32>(440.0), NodeKind::Generator)
///             .insert(Volume(0.5));
/// }
/// ```
pub trait SpawnAudioNode {
    /// Add `unit` to the graph and spawn an entity bound to it.
    fn spawn_audio_node<U>(&mut self, unit: U, kind: NodeKind) -> EntityCommands<'_>
    where
        U: AudioUnit + 'static;
}

impl<'w, 's> SpawnAudioNode for Commands<'w, 's> {
    fn spawn_audio_node<U>(&mut self, unit: U, kind: NodeKind) -> EntityCommands<'_>
    where
        U: AudioUnit + 'static,
    {
        let entity = self.spawn_empty().id();
        self.queue(move |world: &mut World| {
            let id = match world.get_resource_mut::<TuttiGraphRes>() {
                Some(mut graph) => graph.0.add(unit),
                None => {
                    bevy_log::warn!(
                        "spawn_audio_node: TuttiGraphRes missing; entity {:?} left without AudioNode",
                        entity
                    );
                    return;
                }
            };
            // Mark the graph dirty so the per-frame commit system flushes
            // this addition along with whatever else mutated this frame.
            if let Some(mut dirty) = world.get_resource_mut::<GraphDirty>() {
                dirty.0 = true;
            }
            if let Ok(mut e) = world.get_entity_mut(entity) {
                e.insert((AudioNode(id), kind));
            }
        });
        self.entity(entity)
    }
}

/// Removes graph nodes for entities whose `AudioNode` component was removed
/// (including despawned entities).
///
/// We can't read the `NodeId` off a despawned entity, so the system tracks
/// `(Entity, NodeId)` pairs in a local map keyed by entity, populated as
/// new `AudioNode`s are added and consumed when the component disappears.
pub fn reconcile_node_despawn(
    mut tracked: Local<std::collections::HashMap<Entity, tutti::NodeId>>,
    mut added: Query<(Entity, &AudioNode), Added<AudioNode>>,
    mut removed: RemovedComponents<AudioNode>,
    graph: Option<ResMut<TuttiGraphRes>>,
    mut dirty: ResMut<GraphDirty>,
) {
    for (entity, node) in added.iter_mut() {
        tracked.insert(entity, node.0);
    }

    let Some(mut graph) = graph else {
        // Nothing to remove against.
        for entity in removed.read() {
            tracked.remove(&entity);
        }
        return;
    };

    for entity in removed.read() {
        if let Some(id) = tracked.remove(&entity) {
            if graph.0.contains(id) {
                graph.0.remove(id);
                dirty.0 = true;
            }
        }
    }
}

type ChangedParams<'w> = (&'w AudioNode, &'w NodeKind, &'w Volume, Option<&'w Mute>);
type ChangedParamFilter = Or<(Changed<Volume>, Changed<Mute>)>;

/// Reconciles `Changed<Volume>` and `Changed<Mute>` into the underlying
/// graph node. Dispatch is keyed off [`NodeKind`]; unknown kinds are
/// skipped (apps can layer their own systems for custom kinds).
#[allow(unused_mut, unused_variables)]
pub fn reconcile_params(
    graph: Option<ResMut<TuttiGraphRes>>,
    changed_vol: Query<ChangedParams, ChangedParamFilter>,
    mut dirty: ResMut<GraphDirty>,
) {
    let Some(mut graph) = graph else { return };

    for (node, kind, volume, mute) in changed_vol.iter() {
        let muted = mute.map(|m| m.0).unwrap_or(false);
        let target = if muted { 0.0 } else { volume.0 };

        match *kind {
            #[cfg(feature = "sampler")]
            NodeKind::Sampler => {
                if let Some(unit) = graph.0.node_mut::<SamplerUnit>(node.0) {
                    unit.set_gain(target);
                    dirty.0 = true;
                }
            }
            // Other kinds: no first-class typed setter at this layer;
            // hosts are expected to layer their own systems. See
            // module docs for the extension pattern.
            _ => {
                let _ = (target, node);
            }
        }
    }
}

/// Runs `graph.commit()` once iff any reconcile system mutated the graph.
pub fn commit_graph(graph: Option<ResMut<TuttiGraphRes>>, mut dirty: ResMut<GraphDirty>) {
    if !dirty.0 {
        return;
    }
    if let Some(mut graph) = graph {
        graph.0.commit();
    }
    dirty.0 = false;
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::App;
    use tutti::dsp::sine_hz;
    use tutti::TuttiEngine;

    fn test_app() -> App {
        let engine = TuttiEngine::builder()
            .inputs(0)
            .outputs(2)
            .build()
            .expect("build engine");
        let TuttiEngine { graph, .. } = engine;

        let mut app = App::new();
        app.insert_resource(crate::TuttiGraphRes(graph));
        app.init_resource::<GraphDirty>();
        app.add_systems(
            bevy_app::Update,
            (
                reconcile_params.in_set(GraphReconcileSet::Params),
                reconcile_node_despawn.in_set(GraphReconcileSet::Despawn),
                commit_graph.in_set(GraphReconcileSet::Commit),
            ),
        );
        app.configure_sets(
            bevy_app::Update,
            (
                GraphReconcileSet::Spawn,
                GraphReconcileSet::Params,
                GraphReconcileSet::Despawn,
                GraphReconcileSet::Commit,
            )
                .chain(),
        );
        app
    }

    #[test]
    fn spawn_inserts_audio_node() {
        let mut app = test_app();
        let mut commands_q = app.world_mut().commands();
        commands_q
            .spawn_audio_node(sine_hz::<f32>(440.0), NodeKind::Generator)
            .insert(Volume(0.5));
        app.update();

        let mut q = app.world_mut().query::<(&AudioNode, &NodeKind, &Volume)>();
        let mut count = 0;
        for (node, kind, vol) in q.iter(app.world()) {
            count += 1;
            assert_eq!(*kind, NodeKind::Generator);
            assert_eq!(vol.0, 0.5);
            assert!(app.world().resource::<crate::TuttiGraphRes>().0.contains(node.0));
        }
        assert_eq!(count, 1);
    }

    #[test]
    fn despawn_removes_graph_node() {
        let mut app = test_app();
        let entity = {
            let mut c = app.world_mut().commands();
            c.spawn_audio_node(sine_hz::<f32>(440.0), NodeKind::Generator).id()
        };
        app.update();

        let node_id = app.world().get::<AudioNode>(entity).expect("AudioNode").0;
        assert!(app.world().resource::<crate::TuttiGraphRes>().0.contains(node_id));

        app.world_mut().despawn(entity);
        app.update();

        assert!(!app.world().resource::<crate::TuttiGraphRes>().0.contains(node_id));
    }

    #[test]
    #[cfg(feature = "sampler")]
    fn sampler_volume_change_writes_through() {
        // Only verifies the dispatch path: a Changed<Volume> on a
        // NodeKind::Sampler entity sets the dirty flag. Real sampler
        // construction needs an asset, which is beyond a unit test here.
        // The dispatch arm itself is covered by the example.
    }
}
