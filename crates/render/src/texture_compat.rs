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
        if !self.alternate {
            // Both stages resolve the same WESL modules and must see the same capabilities.
            descriptor.vertex.shader_defs.push("SINGLE_SAMPLER".into());
            if let Some(fragment) = &mut descriptor.fragment {
                fragment.shader_defs.push("SINGLE_SAMPLER".into());
            }
        }
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
                        *value && gamma.contains(&name.as_ref())
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
            assert_eq!(fragment.shader_defs, vec!["SINGLE_SAMPLER".into()]);
            assert_eq!(
                fragment.targets[0].as_ref().unwrap().blend,
                Some(BlendState::ALPHA_BLENDING)
            );
        }
    }
}

/// Allocates real 2D arrays on GLES, whose target selection otherwise mistakes one layer for
/// D2 and square multiples of six for cube maps. Padding is not part of logical layer indices.
pub(crate) fn d2_array_layers(device: &RenderDevice, width: u32, height: u32, layers: u32) -> u32 {
    d2_array_layers_raw(device.wgpu_device(), width, height, layers)
}

/// Raw-device counterpart for shader-pack resources that do not retain Bevy wrappers.
pub(crate) fn d2_array_layers_raw(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    layers: u32,
) -> u32 {
    array_layers(
        device.adapter_info().backend == wgpu::Backend::Gl,
        width,
        height,
        layers,
        device.limits().max_texture_array_layers,
    )
    .expect("2D array padding must fit the device layer limit")
}

fn array_layers(gl: bool, width: u32, height: u32, layers: u32, limit: u32) -> Option<u32> {
    let mut physical = layers;
    if gl {
        physical = physical.max(2);
        if width == height && physical.is_multiple_of(6) {
            physical = physical.checked_add(1)?;
        }
    }
    (physical <= limit).then_some(physical)
}

/// Pads allocation only, preserving descriptors that are also used for logical upload extents.
pub(crate) fn d2_array_descriptor<'a>(
    device: &RenderDevice,
    descriptor: &wgpu::TextureDescriptor<'a>,
) -> wgpu::TextureDescriptor<'a> {
    let mut descriptor = descriptor.clone();
    let size = &mut descriptor.size;
    size.depth_or_array_layers =
        d2_array_layers(device, size.width, size.height, size.depth_or_array_layers);
    descriptor
}

/// Uploads only the authored slices of a single-mip array, never reading padding from `data`.
pub(crate) fn d2_array_with_data(
    device: &RenderDevice,
    queue: &bevy::render::renderer::RenderQueue,
    descriptor: &wgpu::TextureDescriptor<'_>,
    data: &[u8],
) -> bevy::render::render_resource::Texture {
    use crate::queue_writes::ProfiledQueue as _;
    assert_eq!(descriptor.mip_level_count, 1);
    let mut allocation = d2_array_descriptor(device, descriptor);
    allocation.usage |= wgpu::TextureUsages::COPY_DST;
    let texture = device.create_texture(&allocation);
    queue.profiled_write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(
                descriptor.size.width
                    * descriptor
                        .format
                        .block_copy_size(None)
                        .expect("uncompressed array format"),
            ),
            rows_per_image: Some(descriptor.size.height),
        },
        descriptor.size,
    );
    texture
}

#[cfg(test)]
mod array_tests {
    use super::array_layers;

    #[test]
    fn gles_arrays_cannot_be_mistaken_for_2d_or_cube_targets() {
        for layers in 1..=256 {
            let padded = array_layers(true, 16, 16, layers, 256).unwrap();
            assert!(padded >= layers && padded >= 2 && padded <= 256);
            assert!(!padded.is_multiple_of(6));
            assert_eq!(array_layers(false, 16, 16, layers, 256), Some(layers));
        }
        assert_eq!(array_layers(true, 16, 8, 6, 256), Some(6));
        assert_eq!(array_layers(true, 1, 1, 1, 256), Some(2));
        assert_eq!(array_layers(true, 16, 16, 6, 6), None);
        assert_eq!(array_layers(true, 16, 16, 257, 256), None);
    }
}
