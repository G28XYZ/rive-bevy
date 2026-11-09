//! Validate a .riv without opening a window: --example inspect-riv -- /path/file.riv
use rive_bevy::rive_rs::{self, Instantiate};
fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: inspect-riv /path/file.riv");
    let bytes = std::fs::read(&path).expect("read .riv");
    let file = rive_rs::File::new(&bytes).expect("import .riv");
    let board =
        rive_rs::Artboard::instantiate(&file, rive_rs::Handle::Default).expect("default artboard");
    let mut scene: Box<dyn rive_rs::Scene> = Box::instantiate(&board, rive_rs::Handle::Default)
        .expect("default state machine or animation");
    let mut viewport = rive_rs::Viewport::default();
    viewport.resize(512, 512);
    let mut renderer = rive_rs::Renderer::default();
    scene.advance_and_maybe_draw(
        &mut renderer,
        std::time::Duration::from_millis(16),
        &mut viewport,
    );
    assert!(
        !renderer.scene().encoding().is_empty(),
        "scene emitted no drawing commands"
    );
    println!("Imported and drew {path}");
}
