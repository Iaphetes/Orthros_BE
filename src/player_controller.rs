use crate::movable::{Movable, MoveCommand};
use crate::ownable::{Selectable, Selected, SelectionCircle};
use crate::spawner::UnitType;
use crate::ui::RayBlock;

use bevy::asset::LoadState;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::core_pipeline::Skybox;
use bevy::input::mouse::MouseScrollUnit;
use bevy::input::mouse::MouseWheel;
use bevy::math::Quat;
use bevy::render::render_resource::{TextureViewDescriptor, TextureViewDimension};
use bevy::render::view::RenderLayers;
use bevy::utils::HashMap;
use bevy::window::PrimaryWindow;
use bevy::{core_pipeline::bloom::BloomSettings, prelude::*};
use bevy_rapier3d::prelude::*;

pub enum TechLevel {
    L0,
}
#[derive(Eq, Hash, PartialEq, Clone, Copy)]
pub enum Civilisation {
    Greek,
    // ROMAN,
    // JAPANESE,
}
#[derive(Component, Clone)]
pub enum ContextMenuAction {
    Build(UnitType),
}
#[derive(Component)]
pub struct PlayerInfo {
    pub civilisation: Civilisation,
    pub tech_level: TechLevel,
    pub context_menu_actions: HashMap<UnitType, Vec<ContextMenuAction>>,
}
#[derive(Component)]
pub struct LocalPlayer;
#[repr(usize)]
pub enum RenderLayerMap {
    General = 0,
    Main = 1,
    Minimap = 2,
}
pub struct PlayerController;
#[derive(Event)]
pub struct DeselectEvent;
impl Plugin for PlayerController {
    fn build(&self, app: &mut App) {
        app.add_plugins(CameraController)
            .add_event::<RayHit>()
            .add_event::<DeselectEvent>()
            .add_systems(
                Update,
                (
                    process_mouse,
                    mouse_controller.after(process_mouse),
                    asset_loaded,
                ),
            );
    }
}
#[derive(Resource)]
struct Cubemap {
    is_loaded: bool,
    image_handle: Handle<Image>,
}
struct CameraController;
impl Plugin for CameraController {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, camera_setup)
            .add_systems(Update, camera_controller);
    }
}

#[derive(Component)]
pub struct CameraControllerSettings {
    pub enabled: bool,
    pub initialized: bool,
    pub sensitivity: f32,
    pub key_forward: KeyCode,
    pub key_back: KeyCode,
    pub key_left: KeyCode,
    pub key_right: KeyCode,
    pub rotate_key: KeyCode,
    pub rotation_speed: f32,
    pub key_run: KeyCode,
    pub mouse_key_enable_mouse: MouseButton,
    pub mouse_unit_move_button: MouseButton,
    pub keyboard_key_enable_mouse: KeyCode,
    pub pan_speed: f32,
    pub friction: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub velocity: Vec3,
    pub zoom_min: f32,
    pub zoom_max: f32,
    pub zoom_speed: f32,
}

impl Default for CameraControllerSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            initialized: false,
            sensitivity: 0.5,
            key_forward: KeyCode::KeyW,
            key_back: KeyCode::KeyS,
            key_left: KeyCode::KeyA,
            key_right: KeyCode::KeyD,
            rotate_key: KeyCode::ControlLeft,
            rotation_speed: 0.005,
            key_run: KeyCode::ControlLeft,
            mouse_key_enable_mouse: MouseButton::Left,
            mouse_unit_move_button: MouseButton::Right,
            keyboard_key_enable_mouse: KeyCode::KeyM,
            friction: 0.5,
            pitch: 0.0,
            yaw: 0.0,
            velocity: Vec3::ZERO,
            pan_speed: 4.0,
            zoom_speed: 500.0,
            zoom_min: 5.0,
            zoom_max: 100000.0,
        }
    }
}

pub fn camera_controller(
    time: Res<Time>,
    key_input: Res<ButtonInput<KeyCode>>,
    mut mouse_wheel: EventReader<MouseWheel>,
    mut move_toggled: Local<bool>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    mut query: Query<(&mut Transform, &mut CameraControllerSettings), With<Camera>>,
    rapier_context: Res<RapierContext>,
) {
    let dt = time.delta_seconds();

    if let Ok((mut transform, mut options)) = query.get_single_mut() {
        if !options.initialized {
            let (yaw, pitch, _roll) = transform.rotation.to_euler(EulerRot::YXZ);
            options.yaw = yaw;
            options.pitch = pitch;
            options.initialized = true;
        }
        if !options.enabled {
            return;
        }

        // Handle key input
        let mut axis_input = Vec3::ZERO;
        let mut yaw: f32 = 0.0;
        let mut pitch: f32 = 0.0;
        // let mut roll: f32 = 0.0;

        if key_input.pressed(options.rotate_key) {
            if key_input.pressed(options.key_forward) {
                pitch -= 1.0
            }
            if key_input.pressed(options.key_back) {
                pitch += 1.0
            }
            if key_input.pressed(options.key_right) {
                yaw += 1.0;
            }
            if key_input.pressed(options.key_left) {
                yaw -= 1.0;
            }
        } else {
            if key_input.pressed(options.key_forward) {
                axis_input.z += 1.0;
            }
            if key_input.pressed(options.key_back) {
                axis_input.z -= 1.0;
            }
            if key_input.pressed(options.key_right) {
                axis_input.x += 1.0;
            }
            if key_input.pressed(options.key_left) {
                axis_input.x -= 1.0;
            }
        }

        if key_input.just_pressed(options.keyboard_key_enable_mouse) {
            *move_toggled = !*move_toggled;
        }
        for evt in mouse_wheel.read() {
            match evt.unit {
                MouseScrollUnit::Line => {
                    if (transform.translation.y > options.zoom_min || evt.y < 0.0)
                        && (transform.translation.y < options.zoom_max || evt.y > 0.0)
                    {
                        axis_input.y = -evt.y;
                    }
                }
                MouseScrollUnit::Pixel => {}
            }
        }
        // Apply movement update
        if axis_input != Vec3::ZERO {
            options.velocity = axis_input.normalize()
                * Vec3 {
                    x: options.pan_speed,
                    y: options.zoom_speed,
                    z: options.pan_speed,
                };
        } else {
            let friction = options.friction.clamp(0.0, 1.0);
            options.velocity *= 1.0 - friction;
            if options.velocity.length_squared() < 1e-6 {
                options.velocity = Vec3::ZERO;
            }
        }
        let right = transform.right();

        transform.translation += options.velocity.x * dt * *right
            + options.velocity.y * dt * Vec3::Y
            + options.velocity.z * dt * Vec3::Z;
        if key_input.pressed(options.rotate_key) {
            for (camera, camera_transform) in cameras.iter() {
                // First, compute a ray from the mouse position.
                let (ray_pos, ray_dir) = ray_from_camera_center(camera, camera_transform);
                let intersection: Option<(Entity, RayIntersection)> = rapier_context
                    .cast_ray_and_get_normal(
                        ray_pos,
                        ray_dir,
                        f32::MAX,
                        true,
                        QueryFilter::exclude_solids(QueryFilter::new()),
                    );
                match intersection {
                    Some((_, rayintersection)) => {
                        let rot: Quat = Quat::from_rotation_x(pitch * options.rotation_speed)
                            * Quat::from_rotation_y(yaw * options.rotation_speed);
                        transform.rotate_around(rayintersection.point, rot);
                    }
                    None => {
                        println!("Not rotating");
                    }
                }
            }
        }
    }
}

fn camera_setup(
    mut commands: Commands,
    // mut config: ResMut<GizmoConfig>,
    asset_server: Res<AssetServer>,
) {
    // camera
    // config.depth_bias = 0.0;
    // config.line_perspective = true;
    // config.line_width *= 1.;
    let skybox_handle: Handle<Image> = asset_server.load("textures/skybox/stacked.png");
    commands
        .spawn((
            Camera3dBundle {
                camera: Camera {
                    hdr: true,

                    ..default()
                },
                tonemapping: Tonemapping::BlenderFilmic,
                transform: Transform::from_xyz(0.0, 10.0, 0.0).looking_at(Vec3::ZERO, Vec3::Z),
                ..default()
            },
            Skybox {
                image: skybox_handle.clone(),
                brightness: 1000.0,
            },
            BloomSettings::default(),
            RenderLayers::from_layers(&[
                RenderLayerMap::General as usize,
                RenderLayerMap::Main as usize,
            ]),
        ))
        .insert(CameraControllerSettings::default());
    commands.insert_resource(Cubemap {
        is_loaded: false,
        image_handle: skybox_handle,
    });
}
fn asset_loaded(
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut cubemap: ResMut<Cubemap>,
    mut skyboxes: Query<&mut Skybox>,
) {
    if !cubemap.is_loaded
        && asset_server.get_load_state(&cubemap.image_handle) == Some(LoadState::Loaded)
    {
        let image = images.get_mut(&cubemap.image_handle).unwrap();
        // NOTE: PNGs do not have any metadata that could indicate they contain a cubemap texture,
        // so they appear as one texture. The following code reconfigures the texture as necessary.
        if image.texture_descriptor.array_layer_count() == 1 {
            image.reinterpret_stacked_2d_as_array(
                image.texture_descriptor.size.height / image.texture_descriptor.size.width,
            );
            image.texture_view_descriptor = Some(TextureViewDescriptor {
                dimension: Some(TextureViewDimension::Cube),
                ..default()
            });
        }

        for mut skybox in &mut skyboxes {
            skybox.image = cubemap.image_handle.clone();
        }

        cubemap.is_loaded = true;
    }
}

fn mouse_controller(
    mut selectable: Query<(Entity, &mut Selectable, &Children)>,
    mut selection_circle: Query<&mut Visibility, With<SelectionCircle>>,
    mut selected_entities: Query<(Entity, &Selected)>,
    mut movables: Query<Entity, (With<Selected>, With<Movable>)>,
    mut commands: Commands,
    mut ray_hit_event: EventReader<RayHit>,
    deselect_event: EventReader<DeselectEvent>,
    key_input: Res<ButtonInput<KeyCode>>,
) {
    if !deselect_event.is_empty() {
        println!("Deselection");
        for (sel_entity, _, children) in selectable.iter() {
            for child in children.iter() {
                if let Ok(mut selection_visibility) = selection_circle.get_mut(*child) {
                    *selection_visibility = Visibility::Hidden;
                    commands.entity(sel_entity).remove::<Selected>();
                }
            }
        }
    }
    for hit in ray_hit_event.read() {
        if hit.mouse_key_enable_mouse && selected_entities.get_mut(hit.hit_entity).is_err() {
            if let Ok((_, _select, children)) = selectable.get_mut(hit.hit_entity) {
                for child in children.iter() {
                    if let Ok(mut selection_visibility) = selection_circle.get_mut(*child) {
                        *selection_visibility = Visibility::Visible;
                        commands.entity(hit.hit_entity).insert(Selected {});
                    }
                }
            }
            if !key_input.pressed(KeyCode::ControlLeft) {
                for (sel_entity, _, children) in selectable.iter() {
                    let mut deselect: bool = true;

                    if sel_entity == hit.hit_entity {
                        deselect = false;
                    }
                    if deselect {
                        for child in children.iter() {
                            if let Ok(mut selection_visibility) = selection_circle.get_mut(*child) {
                                *selection_visibility = Visibility::Hidden;
                                commands.entity(sel_entity).remove::<Selected>();
                            }
                        }
                    }
                }
            }
        }

        if hit.mouse_unit_move_button {
            println!("Move");
            let target: Vec2 = Vec2 {
                x: hit.ray_intersection.point.x,
                y: hit.ray_intersection.point.z,
            };

            for entity in movables.iter_mut() {
                commands.entity(entity).remove::<MoveCommand>();
                commands.entity(entity).insert(MoveCommand { target });
            }
        }
    }
}

fn ray_from_mouse_position(
    window: &Window,
    camera: &Camera,
    camera_transform: &GlobalTransform,
) -> (Vec3, Vec3) {
    let mouse_position = window.cursor_position().unwrap_or(Vec2::new(0.0, 0.0));
    let ray: Ray3d = camera
        .viewport_to_world(camera_transform, mouse_position)
        .unwrap();
    (ray.origin, *ray.direction)
}

fn ray_from_camera_center(camera: &Camera, camera_transform: &GlobalTransform) -> (Vec3, Vec3) {
    let mouse_position = Vec2::new(0.0, 0.0);
    let ray: Ray3d = camera
        .viewport_to_world(camera_transform, mouse_position)
        .unwrap();
    (ray.origin, *ray.direction)
}

#[derive(Event)]
pub struct RayHit {
    pub hit_entity: Entity,
    pub mouse_key_enable_mouse: bool,
    pub mouse_unit_move_button: bool,
    pub ray_intersection: RayIntersection,
}
fn handle_select(
    primary: &Window,
    rapier_context: &Res<RapierContext>,
    camera: &Camera,
    camera_transform: &GlobalTransform,
    ray_hit_event: &mut EventWriter<RayHit>,
    mouse_unit_move_button: bool,
    mouse_key_enable_mouse: bool,
    deselect_event: &mut EventWriter<DeselectEvent>,
) {
    let (ray_pos, ray_dir) = ray_from_mouse_position(primary, camera, camera_transform);
    // println!("{:?}", mouse_unit_move_button);
    // Then cast the ray.
    let hit = rapier_context.cast_ray_and_get_normal(
        ray_pos,
        ray_dir,
        f32::MAX,
        true,
        QueryFilter::only_kinematic(), // only_dynamic(),
    );
    //Make also sensor cast...
    // let mut hit_entity: Option<Entity> = None;
    if let Some((hit_entity, ray_intersection)) = hit {
        println!("Send event");
        ray_hit_event.send(RayHit {
            hit_entity,
            mouse_unit_move_button,
            mouse_key_enable_mouse,
            ray_intersection,
        });
    } else {
        deselect_event.send(DeselectEvent);
    }
}
fn handle_unit_move_cmd(
    primary: &Window,
    rapier_context: &Res<RapierContext>,
    camera: &Camera,
    camera_transform: &GlobalTransform,
    ray_hit_event: &mut EventWriter<RayHit>,
    mouse_unit_move_button: bool,
    mouse_key_enable_mouse: bool,
) {
    let (ray_pos, ray_dir) = ray_from_mouse_position(primary, camera, camera_transform);

    let hit = rapier_context.cast_ray_and_get_normal(
        ray_pos,
        ray_dir,
        f32::MAX,
        true,
        QueryFilter::exclude_solids(QueryFilter::new()),
    ); //Make also sensor cast...
    if let Some((hit_entity, ray_intersection)) = hit {
        println!("Send event");
        ray_hit_event.send(RayHit {
            hit_entity,
            mouse_unit_move_button,
            mouse_key_enable_mouse,
            ray_intersection,
        });
    }
}
fn process_mouse(
    mut ray_hit_event: EventWriter<RayHit>,
    mut deselect_event: EventWriter<DeselectEvent>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    camera_options: Query<(&CameraControllerSettings, &Camera, &GlobalTransform)>,
    primary_query: Query<&Window, With<PrimaryWindow>>,
    rapier_context: Res<RapierContext>,
    rayblock: Query<Entity, With<RayBlock>>,
) {
    let Ok(primary) = primary_query.get_single() else {
        return;
    };

    let mut mouse_over_ui: bool = false;
    if !rayblock.is_empty() {
        mouse_over_ui = true;
    }
    if let Ok((options, camera, camera_transform)) = camera_options.get_single() {
        let mouse_key_enable_mouse =
            mouse_button_input.just_pressed(options.mouse_key_enable_mouse);
        let mouse_unit_move_button =
            mouse_button_input.just_pressed(options.mouse_unit_move_button);
        if mouse_over_ui {
            return;
        }
        if mouse_key_enable_mouse {
            handle_select(
                primary,
                &rapier_context,
                camera,
                camera_transform,
                &mut ray_hit_event,
                mouse_unit_move_button,
                mouse_key_enable_mouse,
                &mut deselect_event,
            )
        }
        if mouse_unit_move_button {
            handle_unit_move_cmd(
                primary,
                &rapier_context,
                camera,
                camera_transform,
                &mut ray_hit_event,
                mouse_unit_move_button,
                mouse_key_enable_mouse,
            )
        }
    }
}
