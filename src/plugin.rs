use std::sync::Arc;

use bevy::{
    ecs::batching::BatchingStrategy,
    prelude::*,
    render::{extract_component::ExtractComponentPlugin, Render, RenderApp, RenderSystems},
};
use rive_rs::Instantiate;

use crate::{
    assets::{self, Riv, RivLoader},
    components::{
        LinearAnimation, MissingArtboard, MissingLinearAnimation, MissingStateMachine,
        RiveLinearAnimation, RiveStateMachine, RiveTarget, StateMachine, VelloFragment, Viewport,
    },
    events::{GenericEvent, Input, InputValue},
    node, pointer_events,
};

macro_rules! get_scene_or {
    ( $keyword:tt, $linear_animation:expr, $state_machine:expr ) => {{
        let linear_animation = $linear_animation
            .map(|la| la.map_unchanged(|la| (&mut **la) as &mut dyn rive_rs::Scene));
        let state_machine =
            $state_machine.map(|sm| sm.map_unchanged(|sm| (&mut **sm) as &mut dyn rive_rs::Scene));

        match (linear_animation, state_machine) {
            (Some(linear_animation), None) => linear_animation,
            (None, Some(state_machine)) => state_machine,
            _ => $keyword,
        }
    }};
}

pub(crate) use get_scene_or;

macro_rules! get_or_continue_with_error {
    ( $val:expr, $( $tail:tt )* ) => {
        match $val {
            Some(val) => val,
            None => {
                error!($($tail)*);
                continue;
            }
        }
    };
}

fn insert_deafult_viewports(
    mut commands: Commands,
    query: Query<
        (Entity, &RiveTarget),
        (
            Or<(With<LinearAnimation>, With<StateMachine>)>,
            Without<Viewport>,
        ),
    >,
    image_assets: Res<Assets<Image>>,
) {
    for (entity, image_handle) in &query {
        let mut viewport = Viewport::default();

        if let Some(image) = image_assets.get(&image_handle.0) {
            let size = image.size();
            viewport.resize(size.x, size.y);
        }

        commands.entity(entity).insert(viewport);
    }
}

fn resize_viewports(
    mut query: Query<(&mut Viewport, &RiveTarget)>,
    image_assets: Res<Assets<Image>>,
) {
    for (mut viewport, image_handle) in &mut query {
        if let Some(image) = image_assets.get(&image_handle.0) {
            let size = image.size();
            viewport.resize(size.x, size.y);
        }
    }
}

fn instantiate_linear_animations(
    mut commands: Commands,
    query: Query<
        (
            Entity,
            &LinearAnimation,
            Option<&MissingArtboard>,
            Option<&MissingLinearAnimation>,
        ),
        Without<RiveLinearAnimation>,
    >,
    riv_assets: Res<Assets<assets::Riv>>,
) {
    for (entity, linear_animation, missing_artboard, missing_linear_animation) in &query {
        if let Some(riv) = riv_assets.get(&linear_animation.riv) {
            let artboard =
                match rive_rs::Artboard::instantiate(riv, linear_animation.artboard_handle.clone())
                {
                    Some(artboard) => artboard,
                    None => {
                        if missing_artboard.is_none() {
                            commands.entity(entity).insert(MissingArtboard);

                            error!(
                                "artboard {:?} cannot be found in {:?}",
                                linear_animation.artboard_handle, riv,
                            );
                        }

                        continue;
                    }
                };

            commands.entity(entity).remove::<MissingArtboard>();

            let linear_animation = match rive_rs::LinearAnimation::instantiate(
                &artboard,
                linear_animation.handle.clone(),
            ) {
                Some(linear_animation) => linear_animation,
                None => {
                    if missing_linear_animation.is_none() {
                        commands.entity(entity).insert(MissingLinearAnimation);

                        error!(
                            "linear animation {:?} cannot be found in {:?}",
                            linear_animation.handle, riv,
                        );
                    }

                    continue;
                }
            };

            commands.entity(entity).remove::<MissingLinearAnimation>();

            commands
                .entity(entity)
                .insert(RiveLinearAnimation(linear_animation));
        }
    }
}

fn instantiate_state_machines(
    mut commands: Commands,
    query: Query<
        (
            Entity,
            &StateMachine,
            Option<&MissingArtboard>,
            Option<&MissingStateMachine>,
        ),
        Without<RiveStateMachine>,
    >,
    riv_assets: Res<Assets<assets::Riv>>,
) {
    for (entity, state_machine, missing_artboard, missing_state_machine) in &query {
        if let Some(riv) = riv_assets.get(&state_machine.riv) {
            let artboard =
                match rive_rs::Artboard::instantiate(riv, state_machine.artboard_handle.clone()) {
                    Some(artboard) => artboard,
                    None => {
                        if missing_artboard.is_none() {
                            commands.entity(entity).insert(MissingArtboard);

                            error!(
                                "artboard {:?} cannot be found in {:?}",
                                state_machine.artboard_handle, riv,
                            );
                        }

                        continue;
                    }
                };

            commands.entity(entity).remove::<MissingArtboard>();

            let state_machine =
                match rive_rs::StateMachine::instantiate(&artboard, state_machine.handle.clone()) {
                    Some(state_machine) => state_machine,
                    None => {
                        if missing_state_machine.is_none() {
                            commands.entity(entity).insert(MissingStateMachine);

                            error!(
                                "state machine {:?} cannot be found in {:?}",
                                state_machine.handle, riv,
                            );
                        }

                        continue;
                    }
                };

            commands.entity(entity).remove::<MissingStateMachine>();

            commands
                .entity(entity)
                .insert(RiveStateMachine(state_machine));
        }
    }
}

fn reinstantiate_linear_animations(
    mut commands: Commands,
    mut asset_events: MessageReader<AssetEvent<assets::Riv>>,
    scenes: Query<
        (
            Entity,
            Option<Ref<LinearAnimation>>,
            Option<Ref<StateMachine>>,
            Has<RiveLinearAnimation>,
            Has<RiveStateMachine>,
        ),
        Or<(
            With<LinearAnimation>,
            With<StateMachine>,
            With<RiveLinearAnimation>,
            With<RiveStateMachine>,
        )>,
    >,
) {
    let changed_assets: Vec<_> = asset_events
        .read()
        .filter_map(|event| match event {
            AssetEvent::Modified { id } | AssetEvent::Removed { id } => Some(*id),
            _ => None,
        })
        .collect();

    for (entity, animation, machine, has_animation, has_machine) in &scenes {
        let selection_changed = animation.as_ref().is_some_and(|a| a.is_changed())
            || machine.as_ref().is_some_and(|m| m.is_changed());
        let selection_removed =
            (has_animation && animation.is_none()) || (has_machine && machine.is_none());
        let asset_changed = animation
            .as_ref()
            .is_some_and(|a| changed_assets.contains(&a.riv.id()))
            || machine
                .as_ref()
                .is_some_and(|m| changed_assets.contains(&m.riv.id()));
        if selection_changed || selection_removed || asset_changed {
            commands.entity(entity).remove::<(
                RiveLinearAnimation,
                RiveStateMachine,
                VelloFragment,
                MissingArtboard,
                MissingLinearAnimation,
                MissingStateMachine,
            )>();
        }
    }
}

fn pass_state_machine_input_events(
    mut query: Query<&mut RiveStateMachine>,
    mut input_events: MessageReader<Input>,
) {
    for input in input_events.read() {
        if let Ok(state_machine) = query.get_mut(input.state_machine) {
            match input.value {
                InputValue::Bool(val) => get_or_continue_with_error!(
                    state_machine.get_bool(&input.name),
                    "input with name {:?} cannot be found in {:?}",
                    input.name,
                    input.state_machine,
                )
                .set(val),
                InputValue::Number(val) => get_or_continue_with_error!(
                    state_machine.get_number(&input.name),
                    "input with name {:?} cannot be found in {:?}",
                    input.name,
                    input.state_machine,
                )
                .set(val),
                InputValue::Trigger => get_or_continue_with_error!(
                    state_machine.get_trigger(&input.name),
                    "input with name {:?} cannot be found in {:?}",
                    input.name,
                    input.state_machine,
                )
                .fire(),
            }
        }
    }
}

fn render_rive_scenes(
    time: Res<Time>,
    par_commands: ParallelCommands,
    mut query: Query<(
        Entity,
        Option<&mut RiveLinearAnimation>,
        Option<&mut RiveStateMachine>,
        &mut Viewport,
    )>,
) {
    const MAX_SCENES_PER_CORE: usize = 8;

    let elapsed = time.delta();

    query
        .par_iter_mut()
        .batching_strategy(BatchingStrategy::new().max_batch_size(MAX_SCENES_PER_CORE))
        .for_each(|(entity, linear_animation, state_machine, mut viewport)| {
            let mut renderer = rive_rs::Renderer::default();
            let mut scene = get_scene_or!(return, linear_animation, state_machine);

            par_commands.command_scope(|mut commands| {
                if scene.advance_and_maybe_draw(&mut renderer, elapsed, &mut viewport) {
                    commands
                        .entity(entity)
                        .insert(VelloFragment(Arc::new(renderer.into_scene())));
                } else {
                    commands.entity(entity).remove::<VelloFragment>();
                }
            });
        });
}

fn send_generic_events(
    query: Query<(Entity, &RiveStateMachine)>,
    mut generic_events: MessageWriter<GenericEvent>,
) {
    for (entity, state_machine) in &query {
        for event in state_machine.events() {
            generic_events.write(GenericEvent {
                state_machine: entity,
                name: event.name,
                delay: event.delay,
                properties: event.properties,
            });
        }
    }
}

pub struct RivePlugin;

impl Plugin for RivePlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<Riv>()
            .init_asset_loader::<RivLoader>()
            .add_message::<Input>()
            .add_message::<GenericEvent>()
            .add_systems(
                PreUpdate,
                (
                    (insert_deafult_viewports, resize_viewports).chain(),
                    reinstantiate_linear_animations,
                    instantiate_linear_animations,
                    instantiate_state_machines,
                )
                    .chain(),
            )
            .add_systems(
                PostUpdate,
                (
                    pointer_events::pass,
                    pass_state_machine_input_events,
                    render_rive_scenes,
                    send_generic_events,
                )
                    .chain()
                    .after(bevy::transform::TransformSystems::Propagate)
                    .after(bevy::camera::CameraUpdateSystems),
            )
            .add_plugins(ExtractComponentPlugin::<VelloFragment>::default());
    }

    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .init_resource::<node::VelloContext>()
            .add_systems(
                Render,
                node::render_scenes
                    .in_set(RenderSystems::Render)
                    .before(bevy::render::renderer::render_system),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instance_app() -> (App, Entity) {
        let mut app = App::new();
        let mut assets = Assets::<Riv>::default();
        let handle = assets.add(Riv(rive_rs::File::new(include_bytes!(
            "../assets/rating-animation.riv"
        ))
        .unwrap()));
        app.insert_resource(assets)
            .add_message::<AssetEvent<Riv>>()
            .add_systems(
                Update,
                (reinstantiate_linear_animations, instantiate_state_machines).chain(),
            );
        let entity = app
            .world_mut()
            .spawn(StateMachine {
                riv: handle,
                ..default()
            })
            .id();
        app.update();
        assert!(app.world().get::<RiveStateMachine>(entity).is_some());
        (app, entity)
    }

    #[test]
    fn changing_machine_selection_replaces_the_runtime() {
        let (mut app, entity) = instance_app();
        app.world_mut()
            .get_mut::<StateMachine>(entity)
            .unwrap()
            .handle = rive_rs::Handle::Name("missing-machine".into());
        app.update();
        assert!(app.world().get::<RiveStateMachine>(entity).is_none());
        assert!(app.world().get::<MissingStateMachine>(entity).is_some());
    }

    #[test]
    fn removing_machine_stops_the_runtime_and_drawing() {
        let (mut app, entity) = instance_app();
        app.world_mut()
            .entity_mut(entity)
            .insert(VelloFragment(Arc::new(vello::Scene::new())))
            .remove::<StateMachine>();
        app.update();
        assert!(app.world().get::<RiveStateMachine>(entity).is_none());
        assert!(app.world().get::<VelloFragment>(entity).is_none());
    }

    #[test]
    fn removing_and_readding_selection_in_one_frame_replaces_the_runtime() {
        let (mut app, entity) = instance_app();
        let mut selection = app
            .world_mut()
            .entity_mut(entity)
            .take::<StateMachine>()
            .unwrap();
        selection.handle = rive_rs::Handle::Name("missing-machine".into());
        app.world_mut().entity_mut(entity).insert(selection);
        app.update();
        assert!(app.world().get::<RiveStateMachine>(entity).is_none());
    }

    #[test]
    fn target_added_after_machine_still_gets_a_viewport() {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>()
            .add_systems(Update, insert_deafult_viewports);
        let entity = app.world_mut().spawn(StateMachine::default()).id();
        app.update();
        let image = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(crate::create_rive_image(128, 64));
        app.world_mut().entity_mut(entity).insert(RiveTarget(image));
        app.update();
        let viewport = app
            .world()
            .get::<Viewport>(entity)
            .expect("late target must initialize viewport");
        assert_eq!((viewport.width(), viewport.height()), (128, 64));
    }

    #[test]
    fn hot_reload_invalidates_every_instance_and_ignores_unused_assets() {
        let mut app = App::new();
        app.add_message::<AssetEvent<Riv>>()
            .add_systems(Update, reinstantiate_linear_animations);
        let assets = Assets::<Riv>::default();
        let handle = assets.reserve_handle();
        let unused = assets.reserve_handle();
        let entities: Vec<_> = (0..2)
            .map(|_| {
                app.world_mut()
                    .spawn(StateMachine {
                        riv: handle.clone(),
                        ..default()
                    })
                    .id()
            })
            .collect();
        app.update();
        for &entity in &entities {
            app.world_mut().entity_mut(entity).insert((
                VelloFragment(Arc::new(vello::Scene::new())),
                MissingArtboard,
            ));
        }
        app.world_mut()
            .write_message(AssetEvent::<Riv>::Modified { id: unused.id() });
        app.update();
        assert!(entities
            .iter()
            .all(|&e| app.world().get::<VelloFragment>(e).is_some()));
        app.world_mut()
            .write_message(AssetEvent::<Riv>::Modified { id: handle.id() });
        app.update();
        for entity in entities {
            assert!(app.world().get::<VelloFragment>(entity).is_none());
            assert!(app.world().get::<MissingArtboard>(entity).is_none());
        }
    }
}
