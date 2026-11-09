use std::collections::HashMap;

use bevy::{
    camera::{visibility::RenderLayers, NormalizedRenderTarget, RenderTarget},
    input::{mouse::MouseButtonInput, ButtonState},
    math::Vec3A,
    mesh::{Indices, VertexAttributeValues},
    prelude::*,
    render::render_resource::Face,
    sprite::Anchor,
    window::PrimaryWindow,
};

use crate::{
    components::{
        MeshEntity, RiveLinearAnimation, RiveStateMachine, RiveTarget, SpriteEntity, Viewport,
    },
    plugin::get_scene_or,
};

#[derive(Debug)]
enum CameraType {
    Camera2d,
    Camera3d,
}

// Rive needs the matching pointer_up even after the pointer leaves the target.
#[derive(Default)]
pub(crate) struct PointerCaptures(HashMap<Entity, Entity>);

const OUTSIDE_TARGET: Vec2 = Vec2::splat(-1_000_000.0);

fn camera_window(
    camera: &Camera,
    target: &RenderTarget,
    primary: Option<Entity>,
) -> Option<Entity> {
    if !camera.is_active {
        return None;
    }
    match target.normalize(primary)? {
        NormalizedRenderTarget::Window(window) => Some(window.entity()),
        _ => None,
    }
}

fn get_filter_map_for_sprite<'a>(
    sprite: &'a Sprite,
    anchor: Anchor,
    transform: &GlobalTransform,
    images: &'a Assets<Image>,
    atlases: &'a Assets<TextureAtlasLayout>,
) -> impl Fn(Vec2) -> Option<Vec2> + Copy + 'a {
    let affine = transform.affine();
    let determinant = affine.matrix3.determinant();
    let inverse = (determinant != 0.0 && determinant.is_finite()).then(|| affine.inverse());
    move |world_pos| {
        let inverse = inverse?;
        if !inverse.is_finite() {
            return None;
        }
        let local = inverse.transform_point3(world_pos.extend(0.0)).truncate();
        sprite
            .compute_pixel_space_point(local, anchor, images, atlases)
            .ok()
    }
}

#[derive(Debug)]
struct PointerEventPasser<'e> {
    cursor_moved_events: Vec<&'e CursorMoved>,
    mouse_button_input_events: Vec<&'e MouseButtonInput>,
}

impl<'e> PointerEventPasser<'e> {
    pub fn new(
        cursor_moved_events: &'e mut MessageReader<CursorMoved>,
        mouse_button_input_events: &'e mut MessageReader<MouseButtonInput>,
    ) -> Self {
        Self {
            cursor_moved_events: cursor_moved_events.read().collect(),
            mouse_button_input_events: mouse_button_input_events.read().collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.cursor_moved_events.is_empty() && self.mouse_button_input_events.is_empty()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn pass<F: Fn(Vec2) -> Option<Vec2> + Copy>(
        &mut self,
        filter_map: F,
        window: Entity,
        entity: Entity,
        captures: &mut PointerCaptures,
        windows: &Query<&Window>,
        scene: &mut dyn rive_rs::Scene,
        viewport: &Viewport,
    ) {
        self.cursor_moved_events.retain(|cursor_moved| {
            if cursor_moved.window != window
                || captures
                    .0
                    .get(&window)
                    .is_some_and(|&captured| captured != entity)
            {
                return true;
            }
            let pos = filter_map(cursor_moved.position)
                .or_else(|| (captures.0.get(&window) == Some(&entity)).then_some(OUTSIDE_TARGET));
            if let Some(pos) = pos {
                scene.pointer_move(pos.x, pos.y, viewport);

                false
            } else {
                true
            }
        });

        self.mouse_button_input_events.retain(|mouse_button_input| {
            if mouse_button_input.button != MouseButton::Left
                || mouse_button_input.window != window
                || captures
                    .0
                    .get(&window)
                    .is_some_and(|&captured| captured != entity)
            {
                return true;
            }

            if let Some(pos) = windows
                .get(mouse_button_input.window)
                .ok()
                .and_then(|w| w.cursor_position())
                .and_then(filter_map)
                .or_else(|| {
                    (mouse_button_input.state == ButtonState::Released
                        && captures.0.get(&window) == Some(&entity))
                    .then_some(OUTSIDE_TARGET)
                })
            {
                match mouse_button_input.state {
                    ButtonState::Pressed => {
                        scene.pointer_down(pos.x, pos.y, viewport);
                        captures.0.insert(window, entity);
                    }
                    ButtonState::Released => {
                        scene.pointer_up(pos.x, pos.y, viewport);
                        captures.0.remove(&window);
                    }
                }

                false
            } else {
                true
            }
        });
    }
}

#[derive(Debug)]
struct Triangle {
    vertices: [Vec3; 3],
    uvs: [Vec2; 3],
}

impl Triangle {
    pub fn intersect_to_mesh_uv(&self, ray: Ray3d, cull_mode: Option<Face>) -> Option<(f32, Vec2)> {
        let edge0: Vec3A = (self.vertices[1] - self.vertices[0]).into();
        let edge1: Vec3A = (self.vertices[2] - self.vertices[0]).into();
        let ray_direction: Vec3A = (*ray.direction).into();
        let p_vec = ray_direction.cross(edge1);
        let det: f32 = edge0.dot(p_vec);

        let culled_det = match cull_mode {
            Some(Face::Front) => -det,
            Some(Face::Back) => det,
            None => det.abs(),
        };

        if culled_det < f32::EPSILON {
            return None;
        }

        let det_recip = det.recip();

        let t_vec: Vec3A = (ray.origin - self.vertices[0]).into();
        let u = t_vec.dot(p_vec) * det_recip;

        if !(0.0..=1.0).contains(&u) {
            return None;
        }

        let q_vec = t_vec.cross(edge0);
        let v = ray_direction.dot(q_vec) * det_recip;

        if v < 0.0 || u + v > 1.0 {
            return None;
        }

        let distance = edge1.dot(q_vec) * det_recip;
        if distance < 0.0 {
            return None;
        }

        let uv = self.uvs[1].mul_add(
            Vec2::splat(u),
            self.uvs[2].mul_add(Vec2::splat(v), (1.0 - u - v) * self.uvs[0]),
        );

        Some((distance, uv))
    }
}

#[derive(Debug)]
struct Triangles<'m> {
    mesh: &'m Mesh,
    transform: &'m GlobalTransform,
    i: usize,
}

impl<'m> Triangles<'m> {
    pub fn new(mesh: &'m Mesh, transform: &'m GlobalTransform) -> Self {
        Self {
            mesh,
            transform,
            i: 0,
        }
    }
}

impl Iterator for Triangles<'_> {
    type Item = Triangle;

    fn next(&mut self) -> Option<Self::Item> {
        fn get_nth_triangle<U: Copy>(indices: &[U], n: usize) -> Option<[U; 3]> {
            indices.chunks_exact(3).nth(n)?.try_into().ok()
        }

        let indices: [u32; 3] = self.mesh.indices().and_then(|indices| match indices {
            Indices::U16(indices) => get_nth_triangle(indices, self.i).map(|a| a.map(Into::into)),
            Indices::U32(indices) => get_nth_triangle(indices, self.i),
        })?;

        let vertices = self.mesh.attribute(Mesh::ATTRIBUTE_POSITION)?.as_float3()?;
        let VertexAttributeValues::Float32x2(uvs) = self.mesh.attribute(Mesh::ATTRIBUTE_UV_0)?
        else {
            return None;
        };

        let vertices = [
            vertices.get(indices[0] as usize)?,
            vertices.get(indices[1] as usize)?,
            vertices.get(indices[2] as usize)?,
        ]
        .map(|&a| self.transform.transform_point(Vec3::from_array(a)));
        let uvs = [
            uvs.get(indices[0] as usize)?,
            uvs.get(indices[1] as usize)?,
            uvs.get(indices[2] as usize)?,
        ]
        .map(|&a| Vec2::from_array(a));

        self.i += 1;

        Some(Triangle { vertices, uvs })
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn pass(
    cameras: Query<(
        &Camera,
        &RenderTarget,
        &GlobalTransform,
        Option<&Camera2d>,
        Option<&RenderLayers>,
    )>,
    mut cursor_moved_events: MessageReader<CursorMoved>,
    mut mouse_button_input_events: MessageReader<MouseButtonInput>,
    windows: Query<&Window>,
    primary_window: Query<Entity, With<PrimaryWindow>>,
    mut captures: Local<PointerCaptures>,
    mut scenes: Query<(
        Entity,
        Option<&mut RiveLinearAnimation>,
        Option<&mut RiveStateMachine>,
        &RiveTarget,
        &SpriteEntity,
        &MeshEntity,
        &Viewport,
    )>,
    image_assets: Res<Assets<Image>>,
    sprites: Query<(&GlobalTransform, &Sprite, &Anchor, Option<&RenderLayers>)>,
    atlas_assets: Res<Assets<TextureAtlasLayout>>,
    meshes: Query<(
        &GlobalTransform,
        &Mesh3d,
        &MeshMaterial3d<StandardMaterial>,
        Option<&RenderLayers>,
    )>,
    mesh_assets: Res<Assets<Mesh>>,
    material_assets: Res<Assets<StandardMaterial>>,
) {
    let mut cameras: Vec<_> = cameras
        .iter()
        .filter_map(
            |(camera, target, camera_transform, camera_2d, render_layers)| {
                let window = camera_window(camera, target, primary_window.single().ok())?;
                Some((
                    camera,
                    window,
                    camera_transform,
                    if camera_2d.is_some() {
                        CameraType::Camera2d
                    } else {
                        CameraType::Camera3d
                    },
                    render_layers.cloned().unwrap_or_default(),
                ))
            },
        )
        .collect();
    cameras.sort_by_key(|t| std::cmp::Reverse(t.0.order));
    captures
        .0
        .retain(|window, entity| windows.contains(*window) && scenes.contains(*entity));

    let mut passer =
        PointerEventPasser::new(&mut cursor_moved_events, &mut mouse_button_input_events);

    for (camera, window, camera_transform, camera_type, camera_render_layers) in cameras {
        for (
            entity,
            linear_animation,
            state_machine,
            image_handle,
            sprite_entity,
            mesh_entity,
            viewport,
        ) in &mut scenes
        {
            if passer.is_empty() {
                break;
            }

            let mut scene = get_scene_or!(continue, linear_animation, state_machine);
            let image_dimensions = image_assets
                .get(&image_handle.0)
                .map(|image| image.size().as_vec2())
                .unwrap_or_default();

            match camera_type {
                CameraType::Camera2d => {
                    let Some((transform, sprite, anchor, render_layers)) = sprite_entity
                        .entity
                        .and_then(|entity| sprites.get(entity).ok())
                    else {
                        continue;
                    };

                    if !camera_render_layers.intersects(&render_layers.cloned().unwrap_or_default())
                    {
                        continue;
                    }

                    passer.pass(
                        |pos| {
                            if !camera.logical_viewport_rect()?.contains(pos) {
                                return None;
                            }
                            camera
                                .viewport_to_world(camera_transform, pos)
                                .ok()
                                .map(|ray| ray.origin.truncate())
                                .and_then(get_filter_map_for_sprite(
                                    sprite,
                                    *anchor,
                                    transform,
                                    &image_assets,
                                    &atlas_assets,
                                ))
                        },
                        window,
                        entity,
                        &mut captures,
                        &windows,
                        &mut *scene,
                        viewport,
                    );
                }
                CameraType::Camera3d => {
                    let Some((transform, mesh_handle, material_handle, render_layers)) =
                        mesh_entity
                            .entity
                            .and_then(|entity| meshes.get(entity).ok())
                    else {
                        continue;
                    };

                    let Some(mesh) = mesh_assets.get(&mesh_handle.0) else {
                        continue;
                    };

                    let Some(material) = material_assets.get(&material_handle.0) else {
                        continue;
                    };

                    if !camera_render_layers.intersects(&render_layers.cloned().unwrap_or_default())
                    {
                        continue;
                    }

                    let triangles: Vec<_> = Triangles::new(mesh, transform).collect();
                    passer.pass(
                        |pos| {
                            if !camera.logical_viewport_rect()?.contains(pos) {
                                return None;
                            }
                            camera
                                .viewport_to_world(camera_transform, pos)
                                .ok()
                                .and_then(|ray| {
                                    triangles
                                        .iter()
                                        .filter_map(|triangle| {
                                            triangle.intersect_to_mesh_uv(ray, material.cull_mode)
                                        })
                                        .min_by(|a, b| a.0.total_cmp(&b.0))
                                })
                                .map(|(_, pos)| pos * image_dimensions)
                        },
                        window,
                        entity,
                        &mut captures,
                        &windows,
                        &mut *scene,
                        viewport,
                    );
                }
            }
        }
    }

    // The camera or target can disappear between press and release.
    for event in passer.mouse_button_input_events {
        if event.button != MouseButton::Left || event.state != ButtonState::Released {
            continue;
        }
        let Some(entity) = captures.0.remove(&event.window) else {
            continue;
        };
        let Ok((_, animation, machine, _, _, _, viewport)) = scenes.get_mut(entity) else {
            continue;
        };
        let mut scene = get_scene_or!(continue, animation, machine);
        scene.pointer_up(OUTSIDE_TARGET.x, OUTSIDE_TARGET.y, viewport);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default, Resource)]
    struct Trace(Vec<&'static str>);

    impl rive_rs::scene::Scene<rive_rs::Renderer> for Trace {
        fn width(&self) -> f32 {
            100.0
        }
        fn height(&self) -> f32 {
            100.0
        }
        fn name(&self) -> &str {
            "test"
        }
        fn r#loop(&self) -> rive_rs::Loop {
            unreachable!()
        }
        fn is_translucent(&self) -> bool {
            false
        }
        fn duration(&self) -> Option<std::time::Duration> {
            None
        }
        fn pointer_down(&mut self, _: f32, _: f32, _: &rive_rs::Viewport) {
            self.0.push("down");
        }
        fn pointer_move(&mut self, _: f32, _: f32, _: &rive_rs::Viewport) {
            self.0.push("move");
        }
        fn pointer_up(&mut self, _: f32, _: f32, _: &rive_rs::Viewport) {
            self.0.push("up");
        }
        fn advance_and_apply(&mut self, _: std::time::Duration) -> bool {
            unreachable!()
        }
        fn draw(&self, _: &mut rive_rs::Renderer) {
            unreachable!()
        }
        fn advance_and_maybe_draw(
            &mut self,
            _: &mut rive_rs::Renderer,
            _: std::time::Duration,
            _: &mut rive_rs::Viewport,
        ) -> bool {
            unreachable!()
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    fn trace_pointer_events(
        mut moved: MessageReader<CursorMoved>,
        mut buttons: MessageReader<MouseButtonInput>,
        windows: Query<&Window>,
        mut trace: ResMut<Trace>,
        primary: Query<Entity, With<PrimaryWindow>>,
        mut captures: Local<PointerCaptures>,
    ) {
        let window = primary.single().unwrap();
        PointerEventPasser::new(&mut moved, &mut buttons).pass(
            |pos| {
                Rect::new(0.0, 0.0, 100.0, 100.0)
                    .contains(pos)
                    .then_some(pos)
            },
            window,
            window,
            &mut captures,
            &windows,
            &mut *trace,
            &Viewport::default(),
        );
    }

    #[test]
    fn release_outside_the_pressed_sprite_reaches_rive() {
        let mut app = App::new();
        app.init_resource::<Trace>()
            .add_message::<CursorMoved>()
            .add_message::<MouseButtonInput>()
            .add_systems(Update, trace_pointer_events);
        let mut window = Window::default();
        window.set_cursor_position(Some(Vec2::splat(50.0)));
        let window = app.world_mut().spawn((window, PrimaryWindow)).id();
        app.world_mut().write_message(MouseButtonInput {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
            window,
        });
        app.update();
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .set_cursor_position(Some(Vec2::splat(200.0)));
        app.world_mut().write_message(MouseButtonInput {
            button: MouseButton::Left,
            state: ButtonState::Released,
            window,
        });
        app.update();
        assert_eq!(app.world().resource::<Trace>().0, ["down", "up"]);
    }

    #[test]
    fn events_from_another_window_do_not_reach_the_scene() {
        let mut app = App::new();
        app.init_resource::<Trace>()
            .add_message::<CursorMoved>()
            .add_message::<MouseButtonInput>()
            .add_systems(Update, trace_pointer_events);
        app.world_mut().spawn((Window::default(), PrimaryWindow));
        let mut other_window = Window::default();
        other_window.set_cursor_position(Some(Vec2::splat(50.0)));
        let window = app.world_mut().spawn(other_window).id();
        app.world_mut().write_message(MouseButtonInput {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
            window,
        });
        app.world_mut().write_message(CursorMoved {
            window,
            position: Vec2::splat(50.0),
            delta: None,
        });
        app.update();
        assert!(app.world().resource::<Trace>().0.is_empty());
    }

    #[test]
    fn inactive_and_offscreen_cameras_do_not_take_window_events() {
        let mut world = World::new();
        let primary = world.spawn_empty().id();
        let other = world.spawn_empty().id();
        let camera = Camera::default();
        assert_eq!(
            camera_window(&camera, &RenderTarget::default(), Some(primary)),
            Some(primary)
        );
        assert_eq!(
            camera_window(
                &camera,
                &RenderTarget::Window(bevy::window::WindowRef::Entity(other)),
                Some(primary)
            ),
            Some(other)
        );
        assert_eq!(
            camera_window(
                &camera,
                &RenderTarget::Image(Handle::<Image>::default().into()),
                Some(primary)
            ),
            None
        );
        assert_eq!(
            camera_window(
                &Camera {
                    is_active: false,
                    ..default()
                },
                &RenderTarget::default(),
                Some(primary)
            ),
            None
        );
    }

    #[test]
    fn sprite_parent_custom_size_anchor_and_flip_match_rendering() {
        let mut images = Assets::<Image>::default();
        let sprite = Sprite {
            image: images.add(crate::create_rive_image(100, 40)),
            custom_size: Some(Vec2::new(200.0, 80.0)),
            flip_x: true,
            ..default()
        };
        let parent = GlobalTransform::from(Transform::from_xyz(1000.0, 500.0, 0.0));
        let transform = parent.mul_transform(Transform::from_rotation(Quat::from_rotation_z(
            std::f32::consts::FRAC_PI_2,
        )));
        let world = transform.transform_point(Vec3::new(50.0, 20.0, 0.0));
        let point = get_filter_map_for_sprite(
            &sprite,
            Anchor::BOTTOM_LEFT,
            &transform,
            &images,
            &Assets::default(),
        )(world.truncate())
        .unwrap();
        assert!(point.abs_diff_eq(Vec2::new(75.0, 30.0), 0.0001));
    }

    #[test]
    fn zero_scale_sprite_is_not_clickable() {
        assert!(sprite_point(Transform::from_scale(Vec3::ZERO).into(), Vec2::ZERO).is_none());
    }

    fn sprite_point(transform: GlobalTransform, world: Vec2) -> Option<Vec2> {
        let mut images = Assets::<Image>::default();
        let sprite = Sprite::from_image(images.add(crate::create_rive_image(100, 40)));
        get_filter_map_for_sprite(
            &sprite,
            Anchor::CENTER,
            &transform,
            &images,
            &Assets::default(),
        )(world)
    }

    #[test]
    fn rotated_sprite_uses_its_local_coordinates() {
        let transform =
            Transform::from_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2));
        let world = transform
            .transform_point(Vec3::new(40.0, 10.0, 0.0))
            .truncate();
        let point = sprite_point(transform.into(), world).unwrap();
        assert!(point.abs_diff_eq(Vec2::new(90.0, 10.0), 0.0001));
    }

    #[test]
    fn mirrored_sprite_keeps_clickable_bounds() {
        let transform = Transform::from_scale(Vec3::new(-2.0, 1.0, 1.0));
        let point = sprite_point(transform.into(), Vec2::new(-80.0, 10.0));
        assert_eq!(point, Some(Vec2::new(90.0, 10.0)));
    }

    #[test]
    fn triangle_behind_the_ray_is_not_clickable() {
        let triangle = Triangle {
            vertices: [
                Vec3::new(-1.0, -1.0, -2.0),
                Vec3::new(1.0, -1.0, -2.0),
                Vec3::new(0.0, 1.0, -2.0),
            ],
            uvs: [Vec2::ZERO, Vec2::X, Vec2::Y],
        };
        let ray = Ray3d::new(Vec3::ZERO, Dir3::Z);
        assert!(triangle.intersect_to_mesh_uv(ray, None).is_none());
    }
}
