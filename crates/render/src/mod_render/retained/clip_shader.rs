//! Exact-texel clip mask specialization, shared with shader regression tests.

/// Uses per-sample world occlusion on multisampled views.
pub(crate) fn source(msaa: bool, source: &str) -> String {
    if msaa {
        source
            .replace(
                "var world_depth: texture_2d<f32>",
                "var world_depth: texture_multisampled_2d<f32>",
            )
            .replace(
                "position: vec4<f32>)",
                "position: vec4<f32>, @builtin(sample_index) sample: u32)",
            )
            .replace("position.xy), 0)", "position.xy), i32(sample))")
    } else {
        source.to_owned()
    }
}
