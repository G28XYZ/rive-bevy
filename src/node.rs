//! Render Rive textures once per frame, before Bevy submits its camera passes.
use crate::components::VelloScene;
use bevy::{
    prelude::*,
    render::{
        render_asset::RenderAssets,
        render_resource::{TextureFormat, TextureUsages, TextureViewDescriptor},
        renderer::{RenderDevice, RenderQueue},
        texture::GpuImage,
    },
};
use vello::{AaConfig, RenderParams, Renderer, RendererOptions};

#[derive(Resource)]
pub struct VelloContext(std::sync::Mutex<Renderer>);

impl FromWorld for VelloContext {
    fn from_world(world: &mut World) -> Self {
        let device = world.resource::<RenderDevice>();
        Self(std::sync::Mutex::new(
            Renderer::new(device.wgpu_device(), RendererOptions::default())
                .expect("failed to create Vello renderer"),
        ))
    }
}

pub fn render_scenes(
    mut context: ResMut<VelloContext>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    images: Res<RenderAssets<GpuImage>>,
    scenes: Query<&VelloScene>,
) {
    for scene in &scenes {
        let Some(image) = images.get(&scene.image_handle) else {
            continue;
        };
        // Asset preparation can lag the main world's resize by one frame.
        if scene.width == 0
            || scene.height == 0
            || scene.width != image.texture.width()
            || scene.height != image.texture.height()
        {
            continue;
        }
        if image.texture.format() != TextureFormat::Rgba8Unorm
            || !image
                .texture
                .usage()
                .contains(TextureUsages::STORAGE_BINDING)
        {
            error!("Rive target must be created with create_rive_image");
            continue;
        }
        let storage_view = image.texture.create_view(&TextureViewDescriptor {
            format: Some(TextureFormat::Rgba8Unorm),
            ..default()
        });
        if let Err(error) = context.0.get_mut().unwrap().render_to_texture(
            device.wgpu_device(),
            &queue,
            &scene.fragment,
            &storage_view,
            &RenderParams {
                base_color: vello::peniko::Color::TRANSPARENT,
                width: scene.width,
                height: scene.height,
                antialiasing_method: AaConfig::Area,
            },
        ) {
            error!("Rive rendering failed: {error}");
        }
    }
}
