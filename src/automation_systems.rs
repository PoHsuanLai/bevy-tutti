use bevy_ecs::prelude::*;

use crate::components::{AddAutomationLane, AutomationLaneEmitter};
use crate::TuttiEngineResource;

pub fn automation_lane_system(
    mut commands: Commands,
    engine: Option<Res<TuttiEngineResource>>,
    query: Query<(Entity, &AddAutomationLane), Added<AddAutomationLane>>,
) {
    let Some(engine) = engine else { return };

    for (entity, add) in query.iter() {
        let lane = engine.automation_lane(add.envelope.clone());

        let node_id = engine.graph_mut(|net| net.add(lane).id());

        commands
            .entity(entity)
            .remove::<AddAutomationLane>()
            .insert(AutomationLaneEmitter { node_id });

        bevy_log::info!("Automation lane added (entity {entity:?}, node {node_id:?})");
    }
}
