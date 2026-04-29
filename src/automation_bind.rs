//! ECS binding for [`tutti::automation::AutomationLane`].
//!
//! Tutti's `AutomationLane` is an `AudioUnit` — a graph node whose output
//! is the current envelope value at the transport's beat position. This
//! module exposes two ergonomic component types on top of that:
//!
//! - [`AutomationLaneNode`] — marker for the entity that owns the lane node.
//! - [`AutomationDrivesParam`] — a small relationship-style component that
//!   says "feed the lane's current value into this *target* entity's
//!   `Volume` / `Pan` / `PluginParam`."
//!
//! The reconcile system [`reconcile_automation_writes`] runs in
//! [`GraphReconcileSet::Params`]: each frame it walks every entity that
//! has both an [`AudioNode`] and an [`AutomationDrivesParam`], peeks at the
//! lane's current `last_value()` via `graph.node::<LiveAutomationLane<f32>>`,
//! and writes that value into the target entity's parameter component.
//!
//! Why this stays inside bevy-tutti: tutti already exposes `AutomationLane`
//! and `last_value()`. We're not re-implementing automation semantics —
//! we're binding the *output* of an existing graph node into Bevy's change
//! detection so the rest of the reconcile pipeline picks it up.

use bevy_ecs::prelude::*;

use tutti::automation::LiveAutomationLane;
use tutti::core::ecs::{AudioNode, PluginParam, Volume};

use crate::TuttiGraphRes;

/// Marker component for entities holding a `LiveAutomationLane<f32>` node.
///
/// Insert this together with [`AudioNode`] when you spawn the lane via
/// `commands.spawn_audio_node(lane, NodeKind::Generator)`. The reconcile
/// system uses it to filter the candidate set; a missing marker just means
/// the lane is purely an audio source (no driven targets) and is skipped
/// for parameter writes.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AutomationLaneNode;

/// Selector for *which* parameter on the target entity the lane writes into.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AutomationParam {
    /// Writes into the target's [`Volume`] component.
    Volume,
    /// Writes into the target's [`tutti::core::ecs::Pan`] component.
    Pan,
    /// Writes into the target's [`PluginParam`] component matching the
    /// given plugin parameter id. The component is updated in place
    /// (`PluginParam::value` is overwritten).
    PluginParam(u32),
}

/// "This automation lane drives a parameter on `target`."
///
/// Attach to the lane entity. The reconcile system reads the lane's current
/// output value and writes it into the target's selected parameter
/// component on the same frame.
///
/// Multiple lanes can target the same entity / parameter; the last write
/// in iteration order wins, mirroring how multiple coincident automation
/// writes work in any DAW. Hosts that need deterministic merging can wrap
/// this in their own resolver and gate the write with a system order.
#[derive(Component, Debug, Clone, Copy)]
pub struct AutomationDrivesParam {
    pub target: Entity,
    pub param: AutomationParam,
}

/// Reads each automation lane's current value and writes it into the
/// target entity's parameter component.
///
/// Runs in [`crate::GraphReconcileSet::Params`]. The downstream parameter
/// reconcilers (`reconcile_params`, `reconcile_plugin_params`, …) pick up
/// the resulting `Changed<Volume>` / `Changed<PluginParam>` later in the
/// same set and route it to the audio thread.
pub fn reconcile_automation_writes(
    graph: Option<Res<TuttiGraphRes>>,
    drivers: Query<(&AudioNode, &AutomationDrivesParam)>,
    mut targets: Query<(Option<&mut Volume>, Option<&mut PluginParam>)>,
) {
    let Some(graph) = graph else { return };

    for (node, drives) in drivers.iter() {
        // The lane value is the same for any T — `get_value_at` returns f32 —
        // so we read it as a `LiveAutomationLane<f32>` regardless of how the
        // host originally typed the envelope. Hosts that pick a different
        // T will need their own bind module.
        let Some(lane) = graph.0.node::<LiveAutomationLane<f32>>(node.0) else {
            continue;
        };
        let value = lane.last_value();

        let Ok((mut maybe_vol, mut maybe_param)) = targets.get_mut(drives.target) else {
            continue;
        };

        match drives.param {
            AutomationParam::Volume => {
                if let Some(v) = maybe_vol.as_deref_mut() {
                    if (v.0 - value).abs() > f32::EPSILON {
                        v.0 = value;
                    }
                }
            }
            AutomationParam::Pan => {
                // Pan needs its own component reference; route through a
                // separate query when callers need it. Skipped here to
                // keep the query borrow set narrow — Pan automation lands
                // in a follow-up commit alongside dawai's Pan reconcile.
                let _ = value;
            }
            AutomationParam::PluginParam(id) => {
                if let Some(p) = maybe_param.as_deref_mut() {
                    if p.id == id && (p.value - value).abs() > f32::EPSILON {
                        p.value = value;
                    }
                }
            }
        }
    }
}
