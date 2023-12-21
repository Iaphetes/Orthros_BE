use crate::{
    civilisation::{CivilisationBoni, CivilisationBoniMap},
    ownable::Selected,
    player_controller::RayHit,
    resources::{ResourceLevel, ResourceLevels, ResourceSource, ResourceType, ResourceUpdateEvent},
    spawner::{EntityWrapper, UnitInformation, UnitStat, UnitType},
    ActivePlayer, PlayerInfo,
};

use bevy::{prelude::*, time::Stopwatch, transform::commands};

//#[derive(Resource)]
//struct CollectionTick {
//    time: Stopwatch,
//}
#[derive(Eq, PartialEq, Copy, Clone)]
enum CollectorState {
    Collecting,
    Approaching,
    Cancelled,
}
#[derive(Component)]
struct Collector {
    resource: ResourceType,
    resource_entity: EntityWrapper,
    player: EntityWrapper,
    collecting: CollectorState,
}

pub struct ResourceCollection;
#[derive(Event)]
pub struct DeselectEvent;
impl Plugin for ResourceCollection {
    fn build(&self, app: &mut App) {
        app.add_event::<RayHit>()
            .add_systems(Update, (start_collecting, collect))
            .add_event::<ResourceUpdateEvent>();
    }
}

fn start_collecting(
    mut commands: Commands,
    mut selected_entities: Query<(Entity, &UnitInformation), With<Selected>>,
    mut ray_hit_event: EventReader<RayHit>,
    mut resource_sources: Query<(Entity, &ResourceLevel)>,
    main_player: Query<Entity, With<ActivePlayer>>,
) {
    let main_player_entity: Entity = main_player.get_single().unwrap();
    for hit in ray_hit_event.read() {
        if let Ok(_) = resource_sources.get(hit.hit_entity) {
            for (entity, unit_information) in selected_entities.iter() {
                match unit_information.unit_type {
                    UnitType::MiningStation => {
                        commands.entity(entity).insert(Collector {
                            resource: ResourceType::Plotanium, //TODO make adaptive
                            resource_entity: EntityWrapper {
                                entity: hit.hit_entity,
                            },

                            player: EntityWrapper {
                                entity: main_player_entity,
                            },
                            collecting: CollectorState::Approaching,
                        });
                    }
                    _ => {}
                }
            }
        }
    }
}
fn check_collection_state(
    collector_entity: Entity,
    collector: &Collector,
    collector_transform: &Transform,
    resource_location: &Query<&Transform, With<ResourceLevel>>,
    unit_information: &UnitInformation,
) -> CollectorState {
    let mut dist = 0.0;
    if let Ok(resource_transform) = resource_location.get(collector.resource_entity.entity) {
        dist = collector_transform
            .translation
            .distance(resource_transform.translation);
    }

    let mut max_mining_dist: f32 = 0.0;
    for stat in &unit_information.stats.0 {
        if let UnitStat::MaxMiningDist(m) = stat {
            max_mining_dist = *m;
        }
    }
    if collector.collecting != CollectorState::Approaching && max_mining_dist < dist {
        return CollectorState::Cancelled;
    } else if collector.collecting == CollectorState::Approaching && max_mining_dist > dist {
        return CollectorState::Collecting;
    }
    return collector.collecting;
}
fn collect(
    time: Res<Time>,
    mut collectors: Query<(Entity, &mut Collector, &Transform, &UnitInformation)>,
    mut resource_levels: Query<&mut ResourceLevels>,
    resource_location: Query<&Transform, With<ResourceLevel>>,
    mut stopwatch: Local<Stopwatch>,
    mut resource_update_events: EventWriter<ResourceUpdateEvent>,
    mut commands: Commands,
    player_infos: Query<&PlayerInfo>,
    civilisation_boni_map: Res<CivilisationBoniMap>,
) {
    stopwatch.tick(time.delta());
    if stopwatch.elapsed().as_secs() >= 1 {
        stopwatch.reset();
        for (collector_entity, mut collector, collector_transform, unit_information) in
            collectors.iter_mut()
        {
            collector.collecting = check_collection_state(
                collector_entity,
                &collector,
                collector_transform,
                &resource_location,
                unit_information,
            );

            // Calculate collection rate
            let mut rate = 0.0;
            for stat in &unit_information.stats.0 {
                match stat {
                    UnitStat::BaseMiningRate(bmr) => rate += *bmr,
                    UnitStat::BonusMiningRate((t, r)) => {
                        if *t == collector.resource {
                            rate += r
                        }
                    }
                    _ => {}
                }
            }
            if rate <= 0.0 {
                collector.collecting = CollectorState::Cancelled;
                println!("Collector apparantly incapable of mining resources");
            }
            let player_info = player_infos.get(collector.player.entity).unwrap();
            let civilisation_boni = civilisation_boni_map
                .map
                .get(&player_info.civilisation)
                .unwrap();
            for (t, r) in &civilisation_boni.eco_boni.resource_boni {
                if *t == collector.resource {
                    rate += r;
                }
            }
            // End
            if collector.collecting == CollectorState::Collecting {
                match resource_levels.get_mut(collector.player.entity) {
                    Ok(mut resource_level) => {
                        if let Some(resource) = resource_level.0.get_mut(&collector.resource) {
                            *resource += rate as i32; //collector.rate as i32;

                            resource_update_events.send(ResourceUpdateEvent(ResourceLevel {
                                resource_type: ResourceType::Plotanium,
                                amount: *resource,
                            }));
                        }
                    }
                    Err(_) => {
                        println!("Could not find player")
                    }
                }
            } else if collector.collecting == CollectorState::Cancelled {
                commands.entity(collector_entity).remove::<Collector>();
            }
        }
    }
}
