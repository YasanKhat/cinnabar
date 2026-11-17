//! Final instance partition after model/page/layer ordering, with one dense main-pass prefix.
use super::*;

impl ActorRenderScene {
    /// Marks coverage-only actors after rig publication, preserving every payload correspondence.
    /// Bone addresses and relative layer order stay unchanged; final instance indices are reassigned.
    pub fn set_mask_only(&mut self, runtime_ids: Vec<u64>) -> &ActorRenderFrame {
        self.set_mask_only_except(runtime_ids, &[])
    }

    /// Keeps explicitly independent runtime/layer pairs in the main pass when their owner is masked.
    pub fn set_mask_only_except(
        &mut self,
        runtime_ids: Vec<u64>,
        independent: &[(u64, u8)],
    ) -> &ActorRenderFrame {
        let frame = &mut self.frame;
        frame.main_pass_instance_count = None;
        frame.mask_only_runtime_ids = runtime_ids.into();
        if frame.mask_only_runtime_ids.is_empty() {
            return frame;
        }
        let scratch = &mut self.coverage_scratch;
        scratch.partition(
            frame
                .rig
                .manifest
                .iter()
                .map(|entry| (entry.identity.runtime_id, entry.identity.layer)),
            &frame.mask_only_runtime_ids,
            independent,
        );
        frame.main_pass_instance_count = Some(scratch.main_count as u32);
        let order = &scratch.order;
        if order.iter().copied().eq(0..order.len()) {
            return frame;
        }
        frame.rig.instances = order
            .iter()
            .map(|index| frame.rig.instances[*index])
            .collect();
        frame.rig.manifest = order
            .iter()
            .enumerate()
            .map(|(new_index, old_index)| {
                let mut entry = frame.rig.manifest[*old_index].clone();
                entry.instance_index = new_index as u32;
                entry
            })
            .collect();
        frame.instances = order
            .iter()
            .map(|index| frame.instances[*index].clone())
            .collect();
        frame.instance_pages = order
            .iter()
            .map(|index| frame.instance_pages[*index])
            .collect();
        frame.instance_revision = frame.instance_revision.wrapping_add(1);
        frame
    }
}

/// Reuses classification storage; each instance is classified once, without a comparison sort.
#[derive(Debug, Default)]
pub(super) struct CoverageScratch {
    owners: Vec<u64>,
    independent: Vec<(u64, u8)>,
    order: Vec<usize>,
    masked: Vec<usize>,
    main_count: usize,
}

impl CoverageScratch {
    /// Produces a stable main-pass prefix followed by a stable coverage-only suffix.
    fn partition(
        &mut self,
        identities: impl IntoIterator<Item = (u64, u8)>,
        owners: &[u64],
        independent: &[(u64, u8)],
    ) {
        self.owners.clear();
        self.owners.extend_from_slice(owners);
        self.owners.sort_unstable();
        self.owners.dedup();
        self.independent.clear();
        self.independent.extend_from_slice(independent);
        self.independent.sort_unstable();
        self.independent.dedup();
        self.order.clear();
        self.masked.clear();
        for (index, identity) in identities.into_iter().enumerate() {
            if self.owners.binary_search(&identity.0).is_ok()
                && self.independent.binary_search(&identity).is_err()
            {
                self.masked.push(index);
            } else {
                self.order.push(index);
            }
        }
        self.main_count = self.order.len();
        self.order.extend_from_slice(&self.masked);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_matches_stable_reference_for_all_owner_subsets() {
        let identities: Vec<_> = (0..64).map(|i| (i % 8, (i % 5) as u8)).collect();
        let independent = [(3, 1), (1, 2), (3, 1), (7, 0)];
        let mut scratch = CoverageScratch::default();
        for mask in 0..256 {
            let owners: Vec<_> = (0..8)
                .rev()
                .filter(|i| mask & (1 << i) != 0)
                .flat_map(|i| [i, i])
                .collect();
            let masked = |index: &usize| {
                let identity = identities[*index];
                owners.contains(&identity.0) && !independent.contains(&identity)
            };
            let mut expected: Vec<_> = (0..identities.len()).collect();
            expected.sort_by_key(masked);
            let main_count = expected.iter().filter(|index| !masked(index)).count();
            scratch.partition(identities.iter().copied(), &owners, &independent);
            assert_eq!(scratch.order, expected, "owner mask {mask}");
            assert_eq!(scratch.main_count, main_count);
        }
        scratch.partition([], &[], &[]);
        assert!(scratch.order.is_empty());
        assert_eq!(scratch.main_count, 0);
    }

    #[test]
    fn warmed_partition_allocates_nothing_when_membership_changes() {
        let identities: Vec<_> = (0..4096).map(|i| (i % 256, (i % 8) as u8)).collect();
        let owners: Vec<_> = (0..256).collect();
        let independent: Vec<_> = (0..256).map(|i| (i, 0)).collect();
        let mut scratch = CoverageScratch::default();
        // Warm both all-main and all-masked capacities before mixed membership.
        scratch.partition(identities.iter().copied(), &owners, &[]);
        scratch.partition(identities.iter().copied(), &[], &independent);
        let mut classified = 0;
        let (_, allocations) = test_alloc::count(|| {
            for owners in [&owners[..128], &owners[128..], &owners[..]] {
                scratch.partition(
                    identities.iter().copied().inspect(|_| classified += 1),
                    owners,
                    &independent,
                );
            }
        });
        assert_eq!(allocations, 0);
        assert_eq!(scratch.order.len(), identities.len());
        assert_eq!(classified, identities.len() * 3);
    }

    #[test]
    fn coverage_partitions_owner_layers_after_its_independent_companion() {
        let mut scene = ActorRenderScene::default();
        let identity = render_model::ActorRenderIdentity {
            session_id: 1,
            dimension: 0,
            runtime_id: 7,
            spawn_revision: std::num::NonZeroU64::MIN,
            ingress_sequence: 1,
            source_tick: Some(1),
            movement_revision: 1,
            pose_generation: render_api::PoseGeneration::new(1),
            layer: 0,
        };
        let layers = [0, render_model::hats::ACTOR_LAYER_HAT + 2, 3];
        let companion = layers[1];
        scene.frame.rig.manifest = layers
            .into_iter()
            .enumerate()
            .map(|(index, layer)| render_model::ActorDrawManifestEntry {
                identity: render_model::ActorRenderIdentity { layer, ..identity },
                rig: render_model::EntityRigId(0),
                completed_tick: 1,
                reset_generation: 1,
                route: render_model::ActorRigRoute::Compiled,
                instance_index: index as u32,
                previous_bone_base: index as u32,
                current_bone_base: index as u32,
                bone_count: 1,
            })
            .collect();
        scene.frame.rig.instances = (0..3)
            .map(|index| render_model::ActorGpuInstance {
                texture_layer: index,
                ..Default::default()
            })
            .collect();
        scene.frame.instances = (0..3)
            .map(|index| ActorRenderInstance {
                runtime_id: 7,
                position: [index as f32, 0.0, 0.0],
                pitch_radians: 0.0,
                yaw_radians: 0.0,
                head_yaw_radians: 0.0,
                skin_layer: index,
            })
            .collect();
        scene.frame.instance_pages = Arc::from([0, 1, 2]);
        let frame = scene.set_mask_only_except(vec![7], &[(7, companion)]);
        assert_eq!(frame.main_pass_instance_count, Some(1));
        assert_eq!(frame.rig.manifest[0].identity.layer, companion);
        assert_eq!(frame.rig.instances[0].texture_layer, 1);
        assert_eq!(frame.instances[0].skin_layer, 1);
        assert_eq!(frame.instance_pages[0], 1);
        for (index, entry) in frame.rig.manifest.iter().enumerate() {
            assert_eq!(entry.instance_index, index as u32);
        }
        let instances = Arc::clone(&frame.instances);
        let pages = Arc::clone(&frame.instance_pages);
        let revision = frame.instance_revision;
        let frame = scene.set_mask_only_except(vec![7, 7], &[(7, companion)]);
        assert_eq!(frame.main_pass_instance_count, Some(1));
        assert_eq!(frame.instance_revision, revision);
        assert!(Arc::ptr_eq(&instances, &frame.instances));
        assert!(Arc::ptr_eq(&pages, &frame.instance_pages));
    }
}
