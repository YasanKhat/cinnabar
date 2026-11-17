use bevy::prelude::Shader;
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Pod, Zeroable)]
pub(super) struct UiViewportUniform {
    pub(super) viewport_size: [f32; 2],
    /// Seconds since the UI renderer started; animates the item glint.
    pub(super) time_seconds: f32,
    pub(super) glint_strength: f32,
}

/// Builds pure WGSL for the selected device's UI filtering path.
pub(crate) fn source(raw: &str, software_bilinear: bool) -> String {
    let raw = raw.replace(
        "UI_BILINEAR_BODY",
        if software_bilinear {
            SOFTWARE_BILINEAR
        } else {
            "return textureSampleLevel(ui_pages, ui_linear_sampler, uv, page, 0.0);"
        },
    );
    // Keep the GPU style bits owned by the renderer, with no UI-crate dependency or WGSL copy.
    let raw = raw.replace("UI_REPEAT_DECODER", render_api::ui_repeat::WGSL_DECODER);
    let raw = raw.replace("UI_SHADOW_DECODER", render_api::ui_shadow::WGSL);
    let raw = raw.replace(
        "UI_HALF_FLOAT_ROUNDING",
        include_str!("../ui_half_float.wgsl"),
    );
    let raw = raw.replace(
        "UI_TEXTURE_VERTEX",
        include_str!("../ui_texture_vertex.wgsl"),
    );
    raw.replace(
        "UI_STYLE_MODEL_VERTEX",
        &format!("{}u", render_api::UI_STYLE_MODEL_VERTEX),
    )
    .replace(
        "UI_MODEL_LAYER_BITS",
        &format!("{}u", render_model::UI_MODEL_LAYER_BITS),
    )
    .replace(
        "// ACTOR_SHADE_CONSTANTS",
        &format!(
            "const ACTOR_SHADE: array<f32, 5> = array({});",
            render_api::ACTOR_SHADE_COEFFICIENTS
                .map(|value| format!("{value:?}"))
                .join(", ")
        ),
    )
    .replace(
        "GLINT_TINT_RGB",
        &crate::material_shader::glint_tint_source(),
    )
    .replace(
        "UI_STYLE_RGB_COVERAGE",
        &format!("{}u", render_api::UI_STYLE_RGB_COVERAGE),
    )
    .replace(
        "UI_STYLE_ALPHA_TEST",
        &format!("{}u", render_api::UI_STYLE_ALPHA_TEST),
    )
    .replace(
        "UI_STYLE_TERRAIN_MIPS",
        &format!("{}u", render_api::UI_STYLE_TERRAIN_MIPS),
    )
    .replace(
        "UI_STYLE_UNORM16_UV",
        &format!("{}u", render_api::UI_STYLE_UNORM16_UV),
    )
    .replace(
        "UI_STYLE_HALF_FLOAT_XY",
        &format!("{}u", render_api::UI_STYLE_HALF_FLOAT_XY),
    )
    .replace(
        "UI_STYLE_SHADOW",
        &format!("{}u", render_api::ui_shadow::UI_STYLE_SHADOW),
    )
    .replace(
        "UI_STYLE_COVERAGE_PAINT",
        &format!("{}u", render_api::UI_STYLE_COVERAGE_PAINT),
    )
    .replace(
        "UI_STYLE_GLINT",
        &format!("{}u", render_model::UI_STYLE_GLINT),
    )
    .replace(
        "UI_STYLE_SPRITE_COLOR_MASK",
        &format!("{}u", render_api::UI_STYLE_SPRITE_COLOR_MASK),
    )
    .replace(
        "UI_STYLE_COLOR_MASK",
        &format!("{}u", render_model::UI_STYLE_COLOR_MASK),
    )
    .replace(
        "UI_STYLE_ADDITIVE",
        &format!("{}u", render_model::UI_STYLE_ADDITIVE),
    )
    .replace(
        "UI_STYLE_PARTICLE",
        &format!("{}u", render_model::UI_STYLE_PARTICLE),
    )
    .replace(
        "UI_STYLE_BILINEAR",
        &format!("{}u", render_api::UI_STYLE_BILINEAR),
    )
    .replace(
        "UI_STYLE_REPEAT_X",
        &format!("{}u", render_api::ui_repeat::UI_STYLE_REPEAT_X),
    )
    .replace(
        "UI_STYLE_REPEAT_Y",
        &format!("{}u", render_api::ui_repeat::UI_STYLE_REPEAT_Y),
    )
    .replace(
        "UI_STYLE_RADIAL_GRADIENT",
        &format!("{}u", render_model::UI_STYLE_RADIAL_GRADIENT),
    )
    .replace(
        "FONT_STYLE_COVERAGE_GAMMA",
        &format!("{}u", assets::FONT_STYLE_COVERAGE_GAMMA),
    )
    .replace("FONT_STYLE_SDF", &format!("{}u", assets::FONT_STYLE_SDF))
    .replace(
        "MAX_UI_TEXTURE_LAYERS",
        &render_model::MAX_UI_TEXTURE_LAYERS.to_string(),
    )
    .replace(
        "UI_STYLE_REPEAT_PAGE",
        &format!("{}u", render_api::ui_repeat::UI_STYLE_REPEAT_PAGE),
    )
    .replace(
        "FONT_STYLE_OUTLINE_COVERAGE",
        &format!("{}u", assets::FONT_STYLE_OUTLINE_COVERAGE),
    )
    .replace(
        "OUTLINE_FILTER_MAX_SAMPLES",
        &format!(
            "{:?}",
            render_model::font_shading::OUTLINE_FILTER_MAX_SAMPLES as f32
        ),
    )
    .replace(
        "SDF_DISTANCE_SCALE",
        &format!("{:?}", render_model::font_shading::SDF_DISTANCE_SCALE),
    )
    .replace(
        "SDF_DISTANCE_BIAS",
        &format!("{:?}", render_model::font_shading::SDF_DISTANCE_BIAS),
    )
    .replace(
        "SDF_SMOOTHING_SCALE",
        &format!("{:?}", render_model::font_shading::SDF_SMOOTHING_SCALE),
    )
    .replace(
        "SDF_MIN_THRESHOLD",
        &format!("{:?}", render_model::font_shading::SDF_MIN_THRESHOLD),
    )
    .replace(
        "COVERAGE_GAMMA_BASE",
        &format!("{:?}", render_model::font_shading::COVERAGE_GAMMA_BASE),
    )
    .replace("COVERAGE_LUMINANCE", &{
        let [r, g, b] = render_model::font_shading::COVERAGE_LUMINANCE;
        format!("vec3<f32>({r:?}, {g:?}, {b:?})")
    })
}

pub(super) fn from_wgsl(raw: &str, software_bilinear: bool, path: impl Into<String>) -> Shader {
    crate::shader_safety::from_wgsl(source(raw, software_bilinear), path)
}

const SOFTWARE_BILINEAR: &str = r#"
    let size = vec2<i32>(textureDimensions(ui_pages, 0));
    let point = uv * vec2<f32>(size) - vec2(0.5);
    let base = vec2<i32>(floor(point));
    let weight = fract(point);
    let hi = size - vec2(1);
    let a = textureLoad(ui_pages, clamp(base, vec2(0), hi), page, 0);
    let b = textureLoad(ui_pages, clamp(base + vec2(1, 0), vec2(0), hi), page, 0);
    let c = textureLoad(ui_pages, clamp(base + vec2(0, 1), vec2(0), hi), page, 0);
    let d = textureLoad(ui_pages, clamp(base + vec2(1, 1), vec2(0), hi), page, 0);
    return mix(mix(a, b, weight.x), mix(c, d, weight.x), weight.y);
"#;
