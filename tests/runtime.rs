use bevy::{
    prelude::*,
    render::render_resource::{TextureFormat, TextureUsages},
};
use rive_bevy::{
    create_rive_image,
    rive_rs::{self, scene::Scene as _, Instantiate},
};

#[test]
fn imports_advances_and_draws_real_rive_file() {
    let file = rive_rs::File::new(include_bytes!("../assets/rating-animation.riv")).unwrap();
    let board = rive_rs::Artboard::instantiate(&file, rive_rs::Handle::Default).unwrap();
    let mut machine = rive_rs::StateMachine::instantiate(&board, rive_rs::Handle::Default).unwrap();
    let mut rating = machine.get_number("rating").expect("rating input");
    rating.set(4.0);
    assert_eq!(rating.get(), 4.0);
    let mut renderer = rive_rs::Renderer::default();
    let mut viewport = rive_rs::Viewport::default();
    viewport.resize(512, 512);
    assert!(machine.advance_and_maybe_draw(
        &mut renderer,
        std::time::Duration::from_millis(16),
        &mut viewport
    ));
    assert!(!renderer.scene().encoding().is_empty());
}

#[test]
fn malformed_file_returns_error() {
    assert!(rive_rs::File::new(b"not a rive file").is_err());
}

#[test]
fn target_can_be_written_by_vello_and_sampled_by_bevy() {
    let image = create_rive_image(128, 64);
    assert_eq!(image.size(), UVec2::new(128, 64));
    assert_eq!(image.texture_descriptor.format, TextureFormat::Rgba8Unorm);
    assert!(image
        .texture_descriptor
        .usage
        .contains(TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING));
}
