# rive-bevy — Bevy 0.19 port

Fork of [rive-app/rive-bevy](https://github.com/rive-app/rive-bevy), targeting
**Bevy 0.19.1**, **Vello 0.10.0**, and **wgpu 29**. The matching
[rive-rs fork](https://github.com/G28XYZ/rive-rs/tree/bevy-0.19) updates the
Vello backend and pins HarfBuzz to the version required by the C++ runtime.

## Run

A Rust toolchain compatible with Bevy 0.19 and a C++ compiler are required.

```sh
git clone --branch bevy-0.19 https://github.com/G28XYZ/rive-bevy
cd rive-bevy
cargo run --example simple_2d
cargo run --example simple_3d
cargo run --example rive-input
cargo run --example rive-events
```

`simple_2d` displays an interactive rating animation. `rive-input` also accepts
keys 1–5. `rive-events` logs animation events. Escape closes the window.
Add `-- --capture` for an unattended run that saves `/tmp/rive-bevy.png` and exits.

## Integration

Add `RivePlugin` after `DefaultPlugins`. Load a `.riv` with `AssetServer` and
spawn a `StateMachine` or `LinearAnimation` together with a `SceneTarget`.

```rust,no_run
use bevy::prelude::*;
use rive_bevy::{create_rive_image, RiveTarget, SceneTarget, SpriteEntity, StateMachine};

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);
    let image = images.add(create_rive_image(512, 512));
    let sprite = commands.spawn(Sprite::from_image(image.clone())).id();
    commands.spawn((
        StateMachine { riv: assets.load("hud.riv"), ..default() },
        SceneTarget {
            image: RiveTarget(image),
            sprite: SpriteEntity { entity: Some(sprite) },
            ..default()
        },
    ));
}
```

Use `create_rive_image`: ordinary `Image::default()` textures are not compatible
with Vello storage writes. The helper supplies an unorm storage texture and an
sRGB sampling view. Each Rive instance should have its own target image.

Send `events::Input` through `MessageWriter` for bool, number and trigger inputs.
Read `GenericEvent` through `MessageReader` for Rive events. For 3D input, assign
`SceneTarget.mesh` to a `Mesh3d` entity with a `MeshMaterial3d<StandardMaterial>`.
Game rules and physics remain in Bevy.

## Migration and scope

- Bevy messages replace the old buffered event API.
- `RiveTarget` wraps the image handle, since asset handles are no longer components.
- Rendering runs in Bevy's render schedule before camera rendering; it no longer
  uses the removed Render Graph API or the old shared texture atlas.
- Asset reload invalidates all instances of a file, including failed instances.
- Mouse release forwards `pointer_up`.
- Static scenes are drawn again so resized or recreated target textures are filled.
- Original Bevy 0.12 examples are retained under `examples/legacy-bevy-0.12`.
  The shmup, bloom HUD and composite UI demos have not been ported.

The underlying Rive C++ revision is unchanged. This port does not promise support
for newer Rive file features, data binding or scripting. Text support is enabled;
Rive layout is disabled because the upstream Rust repository does not include
its required Yoga submodule. The integration uses the Vello backend, not Rive's
native renderer. Image meshes retain the upstream triangle approximation.
Desktop is the initial target; Web/WASM and mobile have not been validated.

## Checks

```sh
cargo fmt --check
cargo check --all-targets --locked
cargo test --lib --test runtime --locked
cargo run --example simple_2d -- --capture
```

MIT license; original upstream attribution is retained in `LICENSE`.

A proposed CI workflow is stored at `ci/bevy-0.19.yml`. It is not installed in
GitHub Actions: the publishing token lacks the `workflow` scope. The inherited
upstream workflow still targets the old toolchain and is not a validation of
this port.

Local validation on macOS arm64 / Metal (2026-11-09): all targets checked,
four tests passed, and both 2D and 3D examples rendered successfully. The
unattended examples sent rating=4 through Bevy messages, checked the runtime
value, and captured the visible four-star result. The project's
`kyatto-ninden-teyandee.riv` and `armored-yattaro.riv` also imported and emitted
Vello drawing commands using `inspect-riv`; this is not a full visual audit
of every animation in those files.
