use super::*;
use meshing::PackedBiomeRecord;
use render_scene::ChunkUploadPriority;

/// Repeats one legal quad to isolate tint collection costs from fixture construction.
fn instance(faces: usize) -> ChunkRenderInstance {
    let quad =
        PackedLiquidQuad::try_pack([0, 0, 0], Face::PositiveY, [224; 4], 0, 0, [0; 2], false)
            .unwrap();
    let key = SubChunkKey::new(0, 0, 0, 0);
    ChunkRenderInstance {
        cube_layout: CubeQuadLayout::default(),
        key,
        origin: chunk_origin(key),
        generation: 1,
        cube_quads: Arc::from([]),
        cube_lighting: Arc::from([]),
        model_refs: Arc::from([]),
        model_lighting: Arc::from([]),
        model_draw_refs: Arc::from([]),
        transparent_model_draw_refs: Arc::from([]),
        liquid_lighting: Arc::from([]),
        liquid_quads: vec![quad; faces].into(),
        has_depth_liquid: false,
        has_transparent_liquid: true,
        depth_liquid_start: None,
        biome: PackedBiomeRecord::fallback(),
        tint_identity: ChunkBiomeTintIdentity::default(),
        priority: ChunkUploadPriority::new(0.0),
        token: None,
        publication_permit: None,
    }
}

/// Constructs the exact address range consumed by a group's transparent faces.
fn build(instance: &ChunkRenderInstance, tints: &ChunkBiomeTints) -> TransparentGroupInput {
    let count = instance
        .depth_liquid_start
        .unwrap_or(instance.liquid_quads.len() as u32);
    let identity =
        TransparentAllocationIdentity::new(instance.key, 1, 0..count * 4, 0..count * 2, 0);
    build_transparent_group(instance, identity, tints).unwrap()
}

#[test]
fn uniform_tint_collection_does_not_grow_with_face_count() {
    let tints = ChunkBiomeTints::default();
    let small = instance(1);
    let large = instance(16384);
    let (_, small_allocations) = test_alloc::count(|| build(&small, &tints));
    let (group, large_allocations) = test_alloc::count(|| build(&large, &tints));
    assert_eq!(group.centroids.len(), large.liquid_quads.len());
    assert_eq!(
        &*group.tint_colors,
        &[tints.entries()[0].water.map(f32::to_bits)]
    );
    assert!(large_allocations <= small_allocations + 1);
    let expected = liquid_quad_centroid(large.origin, large.liquid_quads[0]);
    assert!(
        group
            .centroids
            .iter()
            .all(|value| value.to_array() == expected)
    );
}

#[test]
fn empty_or_depth_only_groups_have_no_transparent_tints() {
    let tints = ChunkBiomeTints::default();
    assert!(build(&instance(0), &tints).tint_colors.is_empty());
    let mut depth = instance(64);
    depth.depth_liquid_start = Some(0);
    let group = build(&depth, &tints);
    assert!(group.centroids.is_empty());
    assert!(group.tint_colors.is_empty());
}

#[test]
fn uniform_tint_missing_from_the_table_is_skipped() {
    let tints = ChunkBiomeTints {
        entries: Arc::from([]),
        identity: ChunkBiomeTintIdentity::default(),
    };
    let group = build(&instance(64), &tints);
    assert_eq!(group.centroids.len(), 64);
    assert!(group.tint_colors.is_empty());
}
