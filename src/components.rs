use std::sync::Arc;

use bevy::render::sync_component::SyncComponent;
use bevy::{prelude::*, render::extract_component::ExtractComponent};
use vello::Scene as SceneFragment;

use crate::Riv;

#[derive(Clone, Component, Debug, Default)]
pub struct LinearAnimation {
    pub riv: Handle<Riv>,
    pub artboard_handle: rive_rs::Handle,
    pub handle: rive_rs::Handle,
    pub sprite_entity: Option<Entity>,
}

#[derive(Component, Debug, Deref, DerefMut)]
pub struct RiveLinearAnimation(pub rive_rs::LinearAnimation);

#[derive(Clone, Component, Debug, Default)]
pub struct StateMachine {
    pub riv: Handle<Riv>,
    pub artboard_handle: rive_rs::Handle,
    pub handle: rive_rs::Handle,
    pub sprite_entity: Option<Entity>,
}

#[derive(Component, Debug, Deref, DerefMut)]
pub struct RiveStateMachine(pub rive_rs::StateMachine);

#[derive(Component, Debug)]
pub(crate) struct MissingArtboard;

#[derive(Component, Debug)]
pub(crate) struct MissingLinearAnimation;

#[derive(Component, Debug)]
pub(crate) struct MissingStateMachine;

#[derive(Clone, Component, Debug, Default, Deref, DerefMut)]
pub struct Viewport(pub rive_rs::Viewport);

#[derive(Clone, Component, Debug, Default, Deref)]
pub struct MeshEntity {
    pub entity: Option<Entity>,
}

#[derive(Clone, Component, Debug, Default, Deref)]
pub struct SpriteEntity {
    pub entity: Option<Entity>,
}

#[derive(Bundle, Debug, Default)]
pub struct SceneTarget {
    pub image: RiveTarget,
    pub sprite: SpriteEntity,
    pub mesh: MeshEntity,
}

#[derive(Component, Deref)]
pub(crate) struct VelloFragment(pub Arc<SceneFragment>);

#[derive(Component)]
pub(crate) struct VelloScene {
    pub fragment: Arc<vello::Scene>,
    pub image_handle: Handle<Image>,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Component, Debug, Default, Deref, DerefMut)]
pub struct RiveTarget(pub Handle<Image>);

impl SyncComponent for VelloFragment {
    type Target = VelloScene;
}

impl ExtractComponent for VelloFragment {
    type QueryData = (
        &'static VelloFragment,
        &'static RiveTarget,
        &'static Viewport,
    );

    type QueryFilter = ();

    type Out = VelloScene;

    fn extract_component(
        (fragment, image, viewport): bevy::ecs::query::QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out> {
        Some(VelloScene {
            fragment: fragment.0.clone(),
            image_handle: image.0.clone(),
            width: viewport.width(),
            height: viewport.height(),
        })
    }
}

/// Allocate a transparent storage texture for Vello. Use this instead of
/// `Image::default()`, whose sRGB format cannot be a Vello storage target.
pub fn create_rive_image(width: u32, height: u32) -> Image {
    use bevy::{
        asset::RenderAssetUsages,
        render::render_resource::{
            Extent3d, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
        },
    };
    assert!(
        width > 0 && height > 0,
        "Rive target dimensions must be nonzero"
    );
    let mut image = Image::new_fill(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage |= TextureUsages::STORAGE_BINDING;
    // Vello writes sRGB-encoded pixels into an unorm storage view. Bevy samples
    // through an sRGB view so its linear-light pipeline receives correct colors.
    image.texture_descriptor.view_formats = &[TextureFormat::Rgba8UnormSrgb];
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        format: Some(TextureFormat::Rgba8UnormSrgb),
        ..default()
    });
    image
}
