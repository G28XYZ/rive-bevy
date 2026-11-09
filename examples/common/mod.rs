#![allow(dead_code)]
use bevy::prelude::*;
use rive_bevy::{
    create_rive_image, RivePlugin, RiveTarget, SceneTarget, SpriteEntity, StateMachine,
};

#[derive(Resource)]
pub struct DemoAsset(pub &'static str);

pub fn app() -> App {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(AssetPlugin {
        file_path: format!("{}/assets", env!("CARGO_MANIFEST_DIR")),
        ..default()
    }))
    .add_plugins(RivePlugin)
    .insert_resource(DemoAsset("rating-animation.riv"))
    .insert_resource(ClearColor(Color::srgb(0.12, 0.16, 0.22)))
    .add_systems(Update, (close_on_esc, smoke_capture));
    app
}

pub fn close_on_esc(keys: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

pub fn setup_2d(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    server: Res<AssetServer>,
    demo: Res<DemoAsset>,
) {
    commands.spawn(Camera2d);
    let image = images.add(create_rive_image(512, 512));
    let sprite = commands.spawn(Sprite::from_image(image.clone())).id();
    commands.spawn((
        StateMachine {
            riv: server.load(demo.0),
            ..default()
        },
        SceneTarget {
            image: RiveTarget(image),
            sprite: SpriteEntity {
                entity: Some(sprite),
            },
            ..default()
        },
    ));
}

/// Optional unattended visual check: --capture writes /tmp/rive-bevy.png,
/// --smoke exits after enough frames for assets and pipelines to finish loading.
pub fn smoke_capture(
    mut commands: Commands,
    mut frame: Local<u32>,
    mut exit: MessageWriter<AppExit>,
    scenes: Query<(Entity, &rive_bevy::RiveStateMachine)>,
    mut inputs: MessageWriter<rive_bevy::events::Input>,
) {
    if !std::env::args().any(|arg| arg == "--smoke" || arg == "--capture") {
        return;
    }
    *frame += 1;
    if *frame == 60 {
        for (entity, _) in &scenes {
            inputs.write(rive_bevy::events::Input {
                state_machine: entity,
                name: "rating".into(),
                value: rive_bevy::events::InputValue::Number(4.0),
            });
        }
    }
    if *frame == 120 {
        assert!(!scenes.is_empty(), "Rive state machine did not load");
        for (_, machine) in &scenes {
            assert_eq!(
                machine.get_number("rating").expect("rating input").get(),
                4.0,
                "Bevy input message did not reach Rive"
            );
        }
        if std::env::args().any(|arg| arg == "--capture") {
            commands
                .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
                .observe(bevy::render::view::screenshot::save_to_disk(
                    "/tmp/rive-bevy.png",
                ));
        }
    }
    if *frame == 180 {
        exit.write(AppExit::Success);
    }
}
