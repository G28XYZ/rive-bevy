//! Interactive Rive state machine rendered into a Bevy sprite.
mod common;
use bevy::prelude::*;
fn main() {
    common::app().add_systems(Startup, common::setup_2d).run();
}
