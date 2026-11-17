//! Desktop GL linking contracts exercised by `cargo test -p render --lib`.
use crate::{desktop_gl::assert_desktop_430, shader_safety, shader_source};

#[path = "nametag_render/shader.rs"]
mod nametag_shader;
#[path = "primitive_shapes/shader.rs"]
mod primitive_shader;

#[test]
fn every_intel_world_program_translates_with_raw_storage_and_bounded_varyings() {
    for name in ["chunk", "model", "liquid", "transparent_terrain"] {
        let raw =
            std::fs::read_to_string(format!("{}/src/{name}.wesl", env!("CARGO_MANIFEST_DIR")))
                .unwrap();
        for flags in [
            &[][..],
            &["PACKED_TERRAIN"][..],
            &["ENHANCED"][..],
            &["PACKED_TERRAIN", "ENHANCED"][..],
        ] {
            let mut definitions = vec!["SINGLE_SAMPLER"];
            definitions.extend_from_slice(flags);
            assert_desktop_430(
                &shader_source::composed(&raw, &definitions),
                &format!("{name}/{definitions:?}"),
            );
        }
    }
    for name in ["actor", "hand_rig"] {
        let raw =
            std::fs::read_to_string(format!("{}/src/{name}.wesl", env!("CARGO_MANIFEST_DIR")))
                .unwrap();
        let raw = shader_safety::from_actor_wesl(
            &raw,
            name,
            render_model::ACTOR_GPU_INSTANCE_WORDS,
            render_model::ACTOR_RIG_VERTEX_WORDS,
        );
        for flags in [
            &[][..],
            &["COMPACT_BONES"][..],
            &["ACTOR_MOTION"][..],
            &["ALPHA_TO_COVERAGE"][..],
            &["ACTOR_MOTION", "ALPHA_TO_COVERAGE"][..],
        ] {
            if name == "hand_rig" && flags.iter().any(|flag| *flag != "COMPACT_BONES") {
                continue;
            }
            assert_desktop_430(
                &shader_source::composed(raw.source.as_str(), flags),
                &format!("{name}/{flags:?}"),
            );
        }
    }
    for name in [
        "terrain_markers",
        "atmosphere",
        "cloud",
        "far_terrain",
        "lightning",
        "weather",
        "particles",
        "dropped_item",
        "entity_shadow",
        "mod_render/primitives",
    ] {
        let raw =
            std::fs::read_to_string(format!("{}/src/{name}.wesl", env!("CARGO_MANIFEST_DIR")))
                .unwrap();
        assert_desktop_430(&shader_source::composed(&raw, &[]), name);
    }
    assert_eq!(
        nametag_shader::NAMETAG_BLEND,
        wgpu::BlendState::ALPHA_BLENDING
    );
    let nametag = nametag_shader::from_wesl(include_str!("nametag.wesl"), "nametag.wesl");
    assert_desktop_430(
        &shader_source::composed(nametag.source.as_str(), &[]),
        "nametag",
    );
    let primitive =
        primitive_shader::shader(include_str!("primitive_shapes/shapes.wesl"), "shapes.wesl");
    assert_desktop_430(
        &shader_source::composed(primitive.source.as_str(), &[]),
        "primitive shapes",
    );
    let drawing = format!(
        "{}\n{}",
        mod_render::material::module(mod_render::material::BUILTIN),
        include_str!("mod_render/retained/drawing.wgsl")
    );
    assert_desktop_430(&drawing, "retained drawing");
}

/// Source-level audit also catches unused declarations removed by WESL pruning.
#[test]
fn non_ui_storage_declarations_are_never_structs() {
    fn audit(dir: &std::path::Path) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                audit(&path);
                continue;
            }
            if !matches!(
                path.extension().and_then(|x| x.to_str()),
                Some("wgsl" | "wesl")
            ) || path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("ui")
            {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            for declaration in source.split(';').filter(|s| s.contains("var<storage")) {
                let ty = declaration.rsplit_once(':').unwrap().1.trim();
                let normalized = ty.split_whitespace().collect::<String>();
                assert!(
                    [
                        "u32",
                        "i32",
                        "f32",
                        "vec2<u32>",
                        "vec4<u32>",
                        "vec4<f32>",
                        "atomic<u32>"
                    ]
                    .iter()
                    .any(|element| normalized
                        .strip_prefix(&format!("array<{element}"))
                        .is_some_and(|tail| tail.starts_with('>') || tail.starts_with(','))),
                    "{}: struct/non-raw storage {ty}",
                    path.display()
                );
            }
        }
    }
    audit(&std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src"));
}

/// All packed face/corner coordinates retain the exact bits of the old literals.
#[test]
fn liquid_bitmask_corners_match_all_literal_coordinates() {
    fn verify(name: &str, expected: [[f32; 2]; 24]) {
        let shader = include_str!("liquid.wesl");
        let body = shader
            .split_once(&format!("fn {name}(index:"))
            .unwrap()
            .1
            .split('}')
            .next()
            .unwrap();
        let masks = body
            .split("0x")
            .skip(1)
            .map(|value| u32::from_str_radix(value.split('u').next().unwrap(), 16).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(masks.len(), 2);
        assert_eq!(body.matches("u >> index) & 1u").count(), 2);
        for face in 0..6 {
            for corner in 0..4 {
                let index = face * 4 + corner;
                for axis in 0..2 {
                    let actual = ((masks[axis] >> index) & 1) as f32;
                    assert_eq!(actual.to_bits(), expected[index][axis].to_bits());
                }
            }
        }
    }
    verify(
        "liquid_face_xz",
        [
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 1.0],
            [0.0, 1.0],
            [1.0, 1.0],
            [1.0, 1.0],
            [1.0, 0.0],
            [1.0, 0.0],
            [0.0, 0.0],
            [0.0, 1.0],
            [1.0, 1.0],
            [1.0, 0.0],
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [1.0, 0.0],
            [1.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 1.0],
            [0.0, 1.0],
            [1.0, 1.0],
            [1.0, 1.0],
        ],
    );
    verify(
        "liquid_base_uv",
        [
            [0.0, 0.0],
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 0.0],
            [0.0, 0.0],
            [0.0, 1.0],
            [1.0, 1.0],
            [1.0, 0.0],
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [0.0, 0.0],
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 0.0],
        ],
    );
}
