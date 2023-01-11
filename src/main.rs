use std::f32::consts::PI;

use bevy::{
    input::mouse::{MouseMotion, MouseWheel},
    prelude::*,
    render::camera::RenderTarget,
    sprite::MaterialMesh2dBundle,
    window::PresentMode,
};
use bevy_inspector_egui::WorldInspectorPlugin;
use rand::{thread_rng, Rng};

pub const CLEAR: Color = Color::rgb(0.1, 0.1, 0.1);

const NUM_BODIES: usize = 10;

const WIDTH: usize = 800;
const HEIGHT: usize = 600;

const GRAVITATIONAL_CONSTANT: f32 = 8.31 * 10e-5;
const DENSITY: f32 = 100.;

#[derive(Component)]
struct MainCamera;

#[derive(Component)]
struct BodyPlaceholder {
    pos: Vec2,
    radius: f32,
    can_place: bool,
}

#[derive(Component)]
struct BodyVelIndicator;

impl BodyPlaceholder {
    fn get_velocity(&self, new_pos: Vec2) -> Vec2 {
        (new_pos - self.pos) * Vec2::new(-1.5, -1.5)
    }
}

#[derive(Component)]
struct Body {
    mass: f32,
    radius: f32,
    ax: f32,
    ay: f32,
    vx: f32,
    vy: f32,
}

impl Body {
    fn get_mass(radius: f32) -> f32 {
        PI * radius * radius * radius * DENSITY
    }
}

fn main() {
    App::new()
        .insert_resource(ClearColor(CLEAR))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            window: WindowDescriptor {
                title: "Gravity simulator".to_string(),
                width: WIDTH as f32,
                height: HEIGHT as f32,
                present_mode: PresentMode::AutoVsync,
                ..default()
            },
            ..default()
        }))
        .add_plugin(WorldInspectorPlugin::new())
        .add_startup_system(setup)
        .add_system(body_update)
        .add_system(body_movement)
        .add_system(cursor_actions)
        .add_system(keyboard_inputs)
        .add_system(camera_zoom)
        .add_system(camera_movement)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn((Camera2dBundle::default(), MainCamera));

    commands.spawn((
        MaterialMesh2dBundle {
            mesh: meshes.add(shape::Circle::new(10.).into()).into(),
            material: materials.add(ColorMaterial::from(Color::rgba(1.0, 1.0, 1.0, 0.5))),
            transform: Transform::from_translation(Vec3::new(0., 0., 10.)),
            visibility: Visibility { is_visible: false },
            ..default()
        },
        BodyPlaceholder {
            pos: Vec2::ZERO,
            radius: 0.,
            can_place: false,
        },
    ));
    commands.spawn((
        MaterialMesh2dBundle {
            mesh: meshes.add(shape::RegularPolygon::new(10., 3).into()).into(),
            material: materials.add(ColorMaterial::from(Color::rgba(1.0, 1.0, 1.0, 0.8))),
            transform: Transform::from_translation(Vec3::new(0., 0., 10.)),
            visibility: Visibility { is_visible: false },
            ..default()
        },
        BodyVelIndicator,
    ));

    spawn_random(commands, meshes, materials);
}

fn body_update(mut query: Query<(&mut Body, &GlobalTransform)>) {
    let mut iter = query.iter_combinations_mut();

    while let Some([(mut body1, transform1), (mut body2, transform2)]) = iter.fetch_next() {
        let delta = transform2.translation() - transform1.translation();
        let distance_sq = delta.length_squared();
        if distance_sq > body1.radius * body1.radius && distance_sq > body2.radius * body2.radius {
            let f = (GRAVITATIONAL_CONSTANT * body1.mass * body2.mass) / distance_sq;
            let fx = delta.x * f;
            let fy = delta.y * f;
            body1.ax += fx / body1.mass;
            body1.ay += fy / body1.mass;

            body2.ax -= fx / body2.mass;
            body2.ay -= fy / body2.mass;
        }
    }
}

fn body_movement(time: Res<Time>, mut query: Query<(&mut Body, &mut Transform)>) {
    for (mut body, mut transfrom) in &mut query {
        body.vx += body.ax * 0.1;
        body.vy += body.ay * 0.1;

        transfrom.translation.x += body.vx * time.delta_seconds();
        transfrom.translation.y += body.vy * time.delta_seconds();

        body.ax = 0.;
        body.ay = 0.;
    }
}

fn cursor_actions(
    buttons: Res<Input<MouseButton>>,
    // need to get window dimensions
    wnds: Res<Windows>,
    // query to get camera transform
    mut q_placeholder: Query<(&mut BodyPlaceholder, &mut Visibility, &mut Transform)>,
    mut q_vel_indicator: Query<(
        &mut BodyVelIndicator,
        &mut Visibility,
        &mut Transform,
        Without<BodyPlaceholder>,
    )>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let (mut placeholder, mut placeholder_visibility, mut placeholder_transform) =
        q_placeholder.single_mut();
    let (_, mut indicator_visibility, mut indicator_transform, _) = q_vel_indicator.single_mut();

    let (camera, camera_transform) = q_camera.single();

    // get the window that the camera is displaying to (or the primary window)
    let wnd = if let RenderTarget::Window(id) = camera.target {
        wnds.get(id).unwrap()
    } else {
        wnds.get_primary().unwrap()
    };
    if let Some(screen_pos) = wnd.cursor_position() {
        let window_size = Vec2::new(wnd.width() as f32, wnd.height() as f32);

        // convert screen position [0..resolution] to ndc [-1..1] (gpu coordinates)
        let ndc = (screen_pos / window_size) * 2.0 - Vec2::ONE;

        // matrix for undoing the projection and camera transform
        let ndc_to_world = camera_transform.compute_matrix() * camera.projection_matrix().inverse();

        // use it to convert ndc to world-space coordinates
        let world_pos = ndc_to_world.project_point3(ndc.extend(-1.0));

        // reduce it to a 2D value
        let pos: Vec2 = world_pos.truncate();
        if buttons.just_pressed(MouseButton::Left) {
            placeholder.pos = pos;

            placeholder_transform.translation = Vec3 {
                x: pos.x,
                y: pos.y,
                z: 10.,
            };
            placeholder.can_place = true;
            placeholder_visibility.is_visible = true;

            indicator_visibility.is_visible = true;
            indicator_transform.translation = Vec3 {
                x: pos.x,
                y: pos.y,
                z: 10.,
            };
        }
        if buttons.pressed(MouseButton::Left) {
            // * Updates placeholder
            if !placeholder.can_place {
                return;
            };
            placeholder.radius += 0.5;
            let scale = placeholder.radius / 10.;
            placeholder_transform.scale = Vec3 {
                x: scale,
                y: scale,
                z: scale,
            };

            let diff = pos - placeholder.pos;
            let mut theta = (diff.y / diff.x).atan();
            match (diff.x >= 0., diff.y >= 0.) {
                (true, true) => theta += PI / 2.,
                (true, false) => theta += PI / 2.,
                (false, true) => theta -= PI / 2.,
                (false, false) => theta -= PI / 2.,
            }
            indicator_transform.scale = Vec3 {
                x: 1.0,
                y: 0.1 * diff.length(),
                z: 1.0,
            };
            indicator_transform.rotation = Quat::from_rotation_z(theta);
        }
        if buttons.just_released(MouseButton::Left) {
            let vel = placeholder.get_velocity(pos);

            let radius = placeholder.radius;
            let mass = Body::get_mass(radius);

            commands.spawn((
                MaterialMesh2dBundle {
                    mesh: meshes.add(shape::Circle::new(radius).into()).into(),
                    material: materials.add(ColorMaterial::from(random_color())),
                    transform: Transform::from_translation(Vec3::new(
                        placeholder_transform.translation.x,
                        placeholder_transform.translation.y,
                        0.,
                    )),
                    ..default()
                },
                Body {
                    mass,
                    radius,
                    ax: 0.,
                    ay: 0.,
                    vx: vel.x,
                    vy: vel.y,
                },
            ));
            placeholder.radius = 0.;
            placeholder.can_place = false;
            placeholder_visibility.is_visible = false;
            placeholder_transform.scale = Vec3::ONE;

            indicator_visibility.is_visible = false;
            indicator_transform.scale = Vec3::ONE;
        }

        if buttons.just_pressed(MouseButton::Right) {
            placeholder.radius = 0.;
            placeholder.can_place = false;
            placeholder_visibility.is_visible = false;
            placeholder_transform.scale = Vec3::ONE;

            indicator_visibility.is_visible = false;
            indicator_transform.scale = Vec3::ONE;
        }
    }
}

fn keyboard_inputs(
    mut commands: Commands,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
    keys: Res<Input<KeyCode>>,
    query: Query<Entity, With<Body>>,
) {
    if keys.just_pressed(KeyCode::Space) {
        for e in query.iter() {
            commands.entity(e).despawn();
        }
    }
    if keys.just_pressed(KeyCode::S) {
        spawn_random(commands, meshes, materials);
    }
}

fn camera_movement(
    mut q_camera: Query<(&mut OrthographicProjection, &mut Transform), With<MainCamera>>,
    mut motion_evr: EventReader<MouseMotion>,
    keys: Res<Input<KeyCode>>,
) {
    if keys.pressed(KeyCode::LControl) {
        let (camera, mut camera_transform) = q_camera.single_mut();
        for ev in motion_evr.iter() {
            camera_transform.translation.x -= ev.delta.x;
            camera_transform.translation.y += ev.delta.y;
        }
    }
}

fn camera_zoom(
    mut q_camera: Query<&mut OrthographicProjection, With<MainCamera>>,
    mut scroll_evr: EventReader<MouseWheel>,
) {
    let mut projection = q_camera.single_mut();
    use bevy::input::mouse::MouseScrollUnit;
    for ev in scroll_evr.iter() {
        match ev.unit {
            MouseScrollUnit::Line => {
                projection.scale += 0.1 * ev.y;
            }
            MouseScrollUnit::Pixel => {
                projection.scale += 0.01 * ev.y;
            }
        }
    }
}

// * Helper functions
fn spawn_random(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let mut rng = thread_rng();
    for _ in 0..NUM_BODIES {
        let radius: f32 = rng.gen_range(2.0..15.0);
        let mass = Body::get_mass(radius);

        let rand_x: f32 = rng.gen_range(-0.5..=0.5);
        let rand_x = rand_x * WIDTH as f32 * 0.4;

        let rand_y: f32 = rng.gen_range(-0.5..=0.5);
        let rand_y = rand_y * HEIGHT as f32 * 0.4;

        commands.spawn((
            MaterialMesh2dBundle {
                mesh: meshes.add(shape::Circle::new(radius).into()).into(),
                material: materials.add(ColorMaterial::from(random_color())),
                transform: Transform::from_translation(Vec3::new(rand_x, rand_y, 0.)),
                ..default()
            },
            Body {
                mass,
                radius,
                ax: 0.,
                ay: 0.,
                vx: 0.,
                vy: 0.,
            },
        ));
    }
}

fn random_color() -> Color {
    let mut rng = thread_rng();
    let r = rng.gen_range(0.0..=1.) as f32;
    let g = rng.gen_range(0.0..=1.) as f32;
    let b = rng.gen_range(0.0..=1.) as f32;

    return Color::rgba(r, g, b, 1.);
}
