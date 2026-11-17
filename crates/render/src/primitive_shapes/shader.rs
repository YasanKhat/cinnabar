//! Shared primitive shader constant injection.
use bevy::prelude::Shader;
// Debug text retains its existing cubic facing independently of actor nametags.
pub(super) const TEXT_ACOS_LINEAR: f32 = 0.87266463;
pub(super) const TEXT_ACOS_CUBIC: f32 = -0.698_131_7;
const TEXT_HORIZONTAL_ZERO: f32 = 0.0001;
/// Injects the shared font-pixel scale and debug-text constants into its shader.
pub(super) fn shader(raw: &str, path: impl Into<String>) -> Shader {
    let source = raw
        .replace(
            "ARROW_KIND_VALUE",
            &(render_api::primitive_shapes::PrimitiveShapeKind::Arrow as u32).to_string(),
        )
        .replace(
            "BOX_KIND_VALUE",
            &(render_api::primitive_shapes::PrimitiveShapeKind::Box as u32).to_string(),
        )
        .replace(
            "ALL_DIMENSIONS_VALUE",
            &render_api::primitive_shapes::PRIMITIVE_ALL_DIMENSIONS.to_string(),
        )
        .replace(
            "ARROW_MIN_LENGTH_SQUARED_VALUE",
            &render_api::primitive_shapes::PRIMITIVE_ARROW_MIN_LENGTH_SQUARED.to_string(),
        )
        .replace(
            "TEXT_SCALE_VALUE",
            &render_model::NAMETAG_BLOCKS_PER_FONT_PIXEL.to_string(),
        )
        .replace("ACOS_LINEAR_VALUE", &TEXT_ACOS_LINEAR.to_string())
        .replace("ACOS_CUBIC_VALUE", &TEXT_ACOS_CUBIC.to_string())
        .replace("HORIZONTAL_ZERO_VALUE", &TEXT_HORIZONTAL_ZERO.to_string());
    crate::shader_safety::from_wesl(source, path)
}
