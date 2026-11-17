//! Shared pinned Naga desktop-backend and Intel link-budget regressions.

/// Exercise the pinned desktop backend, not merely WGSL parsing. GL has independent
/// binding namespaces; allocating within each class mirrors wgpu-hal's GLES remap.
pub(crate) fn assert_desktop_430(source: &str, label: &str) {
    use naga::{AddressSpace, ScalarKind, TypeInner};
    let module = naga::front::wgsl::parse_str(source)
        .unwrap_or_else(|error| panic!("{label}: {}", error.emit_to_string(source)));
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .unwrap_or_else(|error| panic!("{label}: {error:?}"));
    if !label.to_ascii_lowercase().starts_with("ui") {
        assert_raw_storage(&module, label);
        assert_varying_budget(&module, label);
    }
    let inspect = |function: &naga::Function, types: &naga::valid::FunctionInfo| {
        for (handle, expression) in function.expressions.iter() {
            match *expression {
                naga::Expression::Select { condition, .. } => {
                    let vector_condition = matches!(
                        types[condition].ty.inner_with(&module.types),
                        TypeInner::Vector { .. }
                    );
                    let integer_result = matches!(
                        types[handle].ty.inner_with(&module.types),
                        TypeInner::Vector {
                            scalar: naga::Scalar {
                                kind: ScalarKind::Sint | ScalarKind::Uint | ScalarKind::Bool,
                                ..
                            },
                            ..
                        }
                    );
                    assert!(
                        !(vector_condition && integer_result),
                        "{label}: pre-450 integer/bool vector mix: {expression:?}"
                    );
                }
                naga::Expression::Math {
                    fun: naga::MathFunction::CountLeadingZeros,
                    arg,
                    ..
                } => {
                    assert!(
                        !matches!(
                            types[arg].ty.inner_with(&module.types),
                            TypeInner::Vector {
                                scalar: naga::Scalar {
                                    kind: ScalarKind::Sint,
                                    ..
                                },
                                ..
                            }
                        ),
                        "{label}: signed-vector countLeadingZeros"
                    );
                }
                _ => {}
            }
        }
    };
    for (handle, function) in module.functions.iter() {
        inspect(function, &info[handle]);
    }
    for (index, entry) in module.entry_points.iter().enumerate() {
        inspect(&entry.function, info.get_entry_point(index));
    }
    let mut options = naga::back::glsl::Options {
        version: naga::back::glsl::Version::Desktop(430),
        writer_flags: naga::back::glsl::WriterFlags::ADJUST_COORDINATE_SPACE
            | naga::back::glsl::WriterFlags::FORCE_POINT_SIZE,
        ..Default::default()
    };
    let mut next = [0u8; 5];
    for (_, global) in module.global_variables.iter() {
        let Some(binding) = global.binding else {
            continue;
        };
        let class = match global.space {
            AddressSpace::Uniform => 0,
            AddressSpace::Storage { .. } => 1,
            _ => match module.types[global.ty].inner {
                TypeInner::Image {
                    class: naga::ImageClass::Storage { .. },
                    ..
                } => 2,
                TypeInner::Sampler { .. } => 3,
                _ => 4,
            },
        };
        options.binding_map.insert(binding, next[class]);
        next[class] = next[class]
            .checked_add(1)
            .expect("binding count fits GL map");
    }
    for entry in &module.entry_points {
        let pipeline = naga::back::glsl::PipelineOptions {
            shader_stage: entry.stage,
            entry_point: entry.name.clone(),
            multiview: None,
        };
        let mut output = String::new();
        let policies = naga::proc::BoundsCheckPolicies {
            index: naga::proc::BoundsCheckPolicy::Unchecked,
            buffer: naga::proc::BoundsCheckPolicy::Unchecked,
            image_load: naga::proc::BoundsCheckPolicy::ReadZeroSkipWrite,
            binding_array: naga::proc::BoundsCheckPolicy::Unchecked,
        };
        naga::back::glsl::Writer::new(&mut output, &module, &info, &options, &pipeline, policies)
            .and_then(|mut writer| writer.write())
            .unwrap_or_else(|error| panic!("{label}/{}: {error:?}", entry.name));
        assert!(
            !output.contains("GL_EXT_texture_shadow_lod"),
            "{label}/{}: ES shadow extension",
            entry.name
        );
        let used = info.get_entry_point(
            module
                .entry_points
                .iter()
                .position(|candidate| {
                    candidate.name == entry.name && candidate.stage == entry.stage
                })
                .unwrap(),
        );
        let multisampled = module.global_variables.iter().any(|(handle, global)| {
            !used[handle].is_empty()
                && matches!(
                    module.types[global.ty].inner,
                    TypeInner::Image {
                        class: naga::ImageClass::Sampled { multi: true, .. }
                            | naga::ImageClass::Depth { multi: true },
                        ..
                    }
                )
        });
        if !multisampled {
            assert!(
                !output.contains("GL_ARB_shader_texture_image_samples"),
                "{label}/{}: single-sample program requires GL 4.5",
                entry.name
            );
        }
        assert!(
            !output.contains("GL_OES_"),
            "{label}/{}: ES extension",
            entry.name
        );
        assert!(
            !output
                .lines()
                .any(|line| line.contains("invariant") && line.contains("gl_FragCoord")),
            "{label}/{}: invariant fragment position",
            entry.name
        );
        // Optional external driver-front-end check; no tool means no claim of driver validation.
        if std::process::Command::new("glslangValidator")
            .arg("--version")
            .output()
            .is_ok()
        {
            use std::io::Write;
            let stage = match entry.stage {
                naga::ShaderStage::Vertex => "vert",
                naga::ShaderStage::Fragment => "frag",
                naga::ShaderStage::Compute => "comp",
                _ => continue,
            };
            let mut child = std::process::Command::new("glslangValidator")
                .args(["--stdin", "-S", stage])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(output.as_bytes())
                .unwrap();
            let result = child.wait_with_output().unwrap();
            assert!(
                result.status.success(),
                "{label}/{}: {}{}",
                entry.name,
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
        }
    }
}

/// Plain value structs are intentional, but storage blocks must be raw arrays.
fn assert_raw_storage(module: &naga::Module, label: &str) {
    fn contains_struct(module: &naga::Module, ty: naga::Handle<naga::Type>) -> bool {
        match module.types[ty].inner {
            naga::TypeInner::Struct { .. } => true,
            naga::TypeInner::Array { base, .. } => contains_struct(module, base),
            _ => false,
        }
    }
    for (_, global) in module.global_variables.iter() {
        if matches!(global.space, naga::AddressSpace::Storage { .. }) {
            assert!(
                !contains_struct(module, global.ty),
                "{label}: struct SSBO {:?}",
                global.name
            );
        }
    }
}

/// GL also spends resources on builtins. Check scalars, not just the highest slot.
fn assert_varying_budget(module: &naga::Module, label: &str) {
    fn visit(
        module: &naga::Module,
        ty: naga::Handle<naga::Type>,
        binding: Option<&naga::Binding>,
        locations: &mut Vec<u32>,
    ) -> u32 {
        if let Some(naga::Binding::Location { location, .. }) = binding {
            assert!(
                *location <= 10,
                "{label}: location {location} exceeds Intel link budget",
                label = module.types[ty].name.as_deref().unwrap_or("varying")
            );
            locations.push(*location);
            return match module.types[ty].inner {
                naga::TypeInner::Scalar(_) => 1,
                naga::TypeInner::Vector { size, .. } => size as u32,
                _ => panic!("unexpected varying type"),
            };
        }
        if binding.is_some() {
            return 0;
        }
        if let naga::TypeInner::Struct { members, .. } = &module.types[ty].inner {
            members
                .iter()
                .map(|m| visit(module, m.ty, m.binding.as_ref(), locations))
                .sum()
        } else {
            0
        }
    }
    for entry in &module.entry_points {
        let mut locations = Vec::new();
        let components = match entry.stage {
            naga::ShaderStage::Vertex => entry.function.result.as_ref().map_or(0, |r| {
                visit(module, r.ty, r.binding.as_ref(), &mut locations)
            }),
            naga::ShaderStage::Fragment => entry
                .function
                .arguments
                .iter()
                .map(|a| visit(module, a.ty, a.binding.as_ref(), &mut locations))
                .sum(),
            _ => continue,
        };
        assert!(
            components <= 40,
            "{label}/{}: {components} varying components exceeds Intel budget",
            entry.name
        );
        locations.sort_unstable();
        // The five heavy shaders must not waste any slots between their varyings.
        if ["chunk", "model", "liquid", "transparent", "actor"]
            .iter()
            .any(|name| label.starts_with(name))
        {
            assert_eq!(
                locations,
                (0..locations.len() as u32).collect::<Vec<_>>(),
                "{label}/{}: non-contiguous varying locations",
                entry.name
            );
        }
    }
}
