//! Alternate texture views are optional: GLES exposes only each texture's base format.
use bevy::{
    prelude::World,
    render::renderer::{RenderAdapter, RenderDevice},
};

/// Whether the adapter can reinterpret a texture through an alternate compatible format.
pub(crate) fn alternate_views(flags: wgpu::DownlevelFlags) -> bool {
    flags.contains(wgpu::DownlevelFlags::VIEW_FORMATS)
}

/// Reads capabilities from the adapter; wgpu 30 does not expose downlevel flags on Device.
pub(crate) fn world_alternate_views(world: &World) -> bool {
    world.get_resource::<RenderAdapter>().map_or_else(
        || {
            world
                .get_resource::<RenderDevice>()
                .is_none_or(device_alternate_views)
        },
        |adapter| alternate_views(adapter.get_downlevel_capabilities().flags),
    )
}

/// Defensive guard for standalone upload helpers. The GLES backend never advertises VIEW_FORMATS;
/// render-world callers additionally filter the exact adapter flags before requesting raw views.
pub(crate) fn device_alternate_views(device: &RenderDevice) -> bool {
    device.wgpu_device().adapter_info().backend != wgpu::Backend::Gl
}

/// The attachment's actual format when encoded and linear materials cannot share alternate views.
pub(crate) fn encoded_format(format: wgpu::TextureFormat, alternate: bool) -> wgpu::TextureFormat {
    if alternate {
        format.remove_srgb_suffix()
    } else {
        format
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absent_view_formats_keeps_the_texture_format() {
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        assert!(!alternate_views(
            wgpu::DownlevelFlags::all() - wgpu::DownlevelFlags::VIEW_FORMATS
        ));
        assert_eq!(encoded_format(format, false), format);
        assert_eq!(
            encoded_format(format, true),
            wgpu::TextureFormat::Rgba8Unorm
        );
    }
}

/// Specializes ordinary world materials for a single sRGB attachment on downlevel adapters.
/// The native encoded-blend programs require alternate views; without them, use linear blending.
pub(crate) struct ViewFormatSpecializer<S> {
    inner: S,
    alternate: bool,
}

impl<S> ViewFormatSpecializer<S> {
    pub(crate) fn new(inner: S, world: &World) -> Self {
        Self {
            inner,
            alternate: world_alternate_views(world),
        }
    }
}

impl<S: bevy::render::render_resource::Specializer<bevy::render::render_resource::RenderPipeline>>
    bevy::render::render_resource::Specializer<bevy::render::render_resource::RenderPipeline>
    for ViewFormatSpecializer<S>
{
    type Key = S::Key;
    fn specialize(
        &self,
        key: Self::Key,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
    ) -> Result<bevy::render::render_resource::Canonical<Self::Key>, bevy::prelude::BevyError> {
        let key = self.inner.specialize(key, descriptor)?;
        if !self.alternate
            && let Some(fragment) = &mut descriptor.fragment
        {
            let gamma = [
                "NATIVE_GAMMA_BLEND",
                "ACTOR_GAMMA_BLEND",
                "NAMETAG_GAMMA_BLEND",
                "SHAPE_GAMMA",
            ];
            let encoded = fragment
                .shader_defs
                .iter()
                .any(|definition| match definition {
                    bevy::shader::ShaderDefVal::Bool(name, value) => {
                        *value && gamma.contains(&&**name)
                    }
                    _ => false,
                });
            // Untyped definitions are also produced by the built-in material descriptors.
            let encoded = encoded
                || gamma
                    .iter()
                    .any(|name| fragment.shader_defs.contains(&(*name).into()));
            if encoded {
                for target in fragment.targets.iter_mut().flatten() {
                    if target.format == render_scene::SCENE_COLOR_FORMAT.remove_srgb_suffix() {
                        target.format = render_scene::SCENE_COLOR_FORMAT;
                    }
                }
                fragment.shader_defs.retain(|definition| {
                    !gamma.iter().any(|name| {
                        definition == &bevy::shader::ShaderDefVal::from(*name)
                            || definition == &bevy::shader::ShaderDefVal::Bool((*name).into(), true)
                    })
                });
            }
        }
        Ok(key)
    }
}

#[cfg(test)]
mod pipeline_tests {
    use super::*;
    use bevy::{prelude::BevyError, render::render_resource::*};

    #[derive(Clone, Hash, PartialEq, Eq)]
    struct Key;
    impl SpecializerKey for Key {
        const IS_CANONICAL: bool = true;
        type Canonical = Self;
    }
    struct Gamma;
    impl Specializer<RenderPipeline> for Gamma {
        type Key = Key;
        fn specialize(&self, key: Key, _: &mut RenderPipelineDescriptor) -> Result<Key, BevyError> {
            Ok(key)
        }
    }

    #[test]
    fn native_gamma_pipeline_falls_back_to_the_same_srgb_attachment() {
        for definition in [
            "NATIVE_GAMMA_BLEND".into(),
            bevy::shader::ShaderDefVal::Bool("ACTOR_GAMMA_BLEND".into(), true),
            "NAMETAG_GAMMA_BLEND".into(),
            "SHAPE_GAMMA".into(),
        ] {
            let mut descriptor = RenderPipelineDescriptor {
                fragment: Some(FragmentState {
                    shader_defs: vec![definition],
                    targets: vec![Some(ColorTargetState {
                        format: render_scene::SCENE_COLOR_FORMAT.remove_srgb_suffix(),
                        blend: Some(BlendState::ALPHA_BLENDING),
                        write_mask: ColorWrites::ALL,
                    })],
                    ..Default::default()
                }),
                ..Default::default()
            };
            ViewFormatSpecializer {
                inner: Gamma,
                alternate: false,
            }
            .specialize(Key, &mut descriptor)
            .unwrap();
            let fragment = descriptor.fragment.unwrap();
            assert_eq!(
                fragment.targets[0].as_ref().unwrap().format,
                render_scene::SCENE_COLOR_FORMAT
            );
            assert!(fragment.shader_defs.is_empty());
            assert_eq!(
                fragment.targets[0].as_ref().unwrap().blend,
                Some(BlendState::ALPHA_BLENDING)
            );
        }
    }
}
