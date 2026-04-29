use bevy_ecs::prelude::*;

use crate::components::{AddAutomationLane, AutomationLaneEmitter};
use crate::{TransportRes, TuttiGraphRes};

pub fn automation_lane_system(
    mut commands: Commands,
    graph: Option<ResMut<TuttiGraphRes>>,
    transport: Option<Res<TransportRes>>,
    query: Query<(Entity, &AddAutomationLane), Added<AddAutomationLane>>,
) {
    let Some(mut graph) = graph else { return };
    let Some(transport) = transport else { return };

    let mut edited = false;

    for (entity, add) in query.iter() {
        let lane = tutti::automation::AutomationLane::new(
            add.envelope.clone(),
            transport.0.clone(),
        );
        let node_id = graph.0.add(lane);
        edited = true;

        commands
            .entity(entity)
            .remove::<AddAutomationLane>()
            .insert(AutomationLaneEmitter { node_id });

        bevy_log::info!("Automation lane added (entity {entity:?}, node {node_id:?})");
    }

    if edited {
        graph.0.commit();
    }
}
