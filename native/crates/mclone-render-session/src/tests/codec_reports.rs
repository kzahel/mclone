use super::*;

#[test]
fn compile_result_partitions_accepted_and_stale_revisions() {
    let unchanged = RenderSectionKey::new(0, 4, 0);
    let changed = RenderSectionKey::new(0, 5, 0);
    let default_revision = RenderSectionKey::new(0, 6, 0);
    let result = RenderSectionCompileResult {
        target_sections: BTreeSet::from([unchanged, changed, default_revision]),
        section_revisions: BTreeMap::from([(unchanged, 7), (changed, 3)]),
        result: Ok(TexturedRenderSectionBuildReport::default()),
    };

    let acceptance = result.partition_by_revision(|key| {
        if key == changed {
            4
        } else if key == unchanged {
            7
        } else {
            0
        }
    });

    assert_eq!(acceptance.accepted_section_count(), 2);
    assert_eq!(acceptance.stale_section_count(), 1);
    assert_eq!(
        acceptance.accepted_sections,
        BTreeSet::from([unchanged, default_revision])
    );
    assert_eq!(acceptance.stale_sections, BTreeSet::from([changed]));
}

#[test]
fn packed_build_report_roundtrips_section_mesh_payload() {
    let key = RenderSectionKey::new(-2, 5, 7);
    let visibility = VisibilitySet::from_bits(0b101010);
    let report = TexturedRenderSectionBuildReport {
        sections: vec![TexturedRenderSectionMesh {
            key,
            mesh: TexturedVisibleChunkMesh {
                vertices: vec![
                    TexturedChunkVertex {
                        position: [1.0, 2.0, 3.0],
                        uv: [0.25, 0.75],
                        color: [1.0, 0.5, 0.25, 1.0],
                        packed_light: 0x00f0_00f0,
                    },
                    TexturedChunkVertex {
                        position: [4.0, 5.0, 6.0],
                        uv: [0.5, 0.125],
                        color: [0.25, 0.5, 1.0, 1.0],
                        packed_light: 0x000f_000f,
                    },
                ],
                indices: vec![0, 1, 0],
                solid_index_count: 1,
                opaque_index_count: 1,
            },
            visibility,
        }],
        visibility_graph: VisibilityGraphBuildStats {
            build_count: 1,
            total_ms: 2.5,
            worst_ms: 2.5,
        },
    };

    let encoded = encode_textured_render_section_build_report(&report);
    let decoded = decode_textured_render_section_build_report(&encoded).unwrap();

    assert_eq!(decoded, report);
    let summary = summarize_textured_render_section_build_report(&decoded);
    assert_eq!(summary.section_count, 1);
    assert_eq!(summary.non_empty_section_count, 1);
    assert_eq!(summary.vertex_count, 2);
    assert_eq!(summary.index_count, 3);
}

#[test]
fn packed_build_report_rejects_invalid_bytes() {
    assert!(decode_textured_render_section_build_report(b"bad").is_err());

    let report = TexturedRenderSectionBuildReport::default();
    let mut encoded = encode_textured_render_section_build_report(&report);
    encoded.push(1);

    assert!(decode_textured_render_section_build_report(&encoded).is_err());
}
