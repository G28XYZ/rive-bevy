//! Press 1–5 to update a Rive number input. Mouse interaction also works.
mod common;
use bevy::prelude::*;
use rive_bevy::{
    events::{Input, InputValue},
    RiveStateMachine,
};
fn main() {
    common::app()
        .add_systems(Startup, common::setup_2d)
        .add_systems(Update, change_rating)
        .run();
}
fn change_rating(
    keys: Res<ButtonInput<KeyCode>>,
    scenes: Query<Entity, With<RiveStateMachine>>,
    mut inputs: MessageWriter<Input>,
) {
    for (index, key) in [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
    ]
    .into_iter()
    .enumerate()
    {
        if keys.just_pressed(key) {
            for entity in &scenes {
                inputs.write(Input {
                    state_machine: entity,
                    name: "rating".into(),
                    value: InputValue::Number((index + 1) as f32),
                });
            }
        }
    }
}
