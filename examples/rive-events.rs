//! Logs events emitted by the interactive rating animation.
mod common;
use bevy::prelude::*;
use rive_bevy::GenericEvent;
fn main() {
    common::app()
        .insert_resource(common::DemoAsset("rating_animation.riv"))
        .add_systems(Startup, common::setup_2d)
        .add_systems(Update, events)
        .run();
}
fn events(mut events: MessageReader<GenericEvent>) {
    for event in events.read() {
        info!(?event, "Rive event");
    }
}
