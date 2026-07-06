use super::*;

#[test]
fn vis_graph_index_packing_matches_minecraft() {
    assert_eq!(VisGraph::index(0, 0, 0), 0);
    assert_eq!(VisGraph::index(1, 0, 0), 1);
    assert_eq!(VisGraph::index(0, 0, 1), 16);
    assert_eq!(VisGraph::index(0, 1, 0), 256);
    assert_eq!(VisGraph::index(15, 15, 15), 4095);
}

#[test]
fn vis_graph_empty_and_lightly_opaque_sections_are_all_visible() {
    let empty = VisGraph::new().resolve();
    for first in SectionFace::ALL {
        for second in SectionFace::ALL {
            assert_visibility(empty, first, second, true);
        }
    }

    let mut graph = VisGraph::new();
    for x in 0..15 {
        for z in 0..15 {
            graph.set_opaque_local(x, 0, z);
        }
    }
    assert_eq!(graph.opaque_count(), 225);
    let light = graph.resolve();
    for first in SectionFace::ALL {
        for second in SectionFace::ALL {
            assert_visibility(light, first, second, true);
        }
    }
}

#[test]
fn vis_graph_solid_section_has_no_face_visibility() {
    let mut graph = VisGraph::new();
    for y in 0..16 {
        for z in 0..16 {
            for x in 0..16 {
                graph.set_opaque_local(x, y, z);
            }
        }
    }

    let visibility = graph.resolve();
    for first in SectionFace::ALL {
        for second in SectionFace::ALL {
            assert_visibility(visibility, first, second, false);
        }
    }
}

#[test]
fn vis_graph_solid_wall_splits_west_from_east() {
    let mut graph = VisGraph::new();
    for y in 0..16 {
        for z in 0..16 {
            graph.set_opaque_local(8, y, z);
        }
    }

    let visibility = graph.resolve();
    assert_visibility(visibility, SectionFace::West, SectionFace::East, false);
    assert_visibility(visibility, SectionFace::West, SectionFace::Down, true);
    assert_visibility(visibility, SectionFace::East, SectionFace::Down, true);
    assert_visibility(visibility, SectionFace::North, SectionFace::South, true);
    assert_visibility(visibility, SectionFace::Down, SectionFace::Up, true);
}

#[test]
fn vis_graph_tunnel_through_solid_section_only_connects_tunnel_faces() {
    let mut graph = VisGraph::new();
    for y in 0..16 {
        for z in 0..16 {
            for x in 0..16 {
                if y != 8 || z != 8 {
                    graph.set_opaque_local(x, y, z);
                }
            }
        }
    }

    let visibility = graph.resolve();
    assert_visibility(visibility, SectionFace::West, SectionFace::East, true);
    assert_visibility(visibility, SectionFace::West, SectionFace::Down, false);
    assert_visibility(visibility, SectionFace::East, SectionFace::Up, false);
    assert_visibility(visibility, SectionFace::North, SectionFace::South, false);
}

#[test]
fn vis_graph_enclosed_cavity_has_no_outer_face_visibility() {
    let mut graph = VisGraph::new();
    for y in 0..16 {
        for z in 0..16 {
            for x in 0..16 {
                if x != 8 || y != 8 || z != 8 {
                    graph.set_opaque_local(x, y, z);
                }
            }
        }
    }

    let visibility = graph.resolve();
    for first in SectionFace::ALL {
        for second in SectionFace::ALL {
            assert_visibility(visibility, first, second, false);
        }
    }
}

#[test]
fn vis_graph_matches_java_oracle_synthetic_cases() {
    let fixture = include_str!("../../../../../test/fixtures/render/visgraph-synthetic.json");
    let json: serde_json::Value = serde_json::from_str(fixture).unwrap();
    assert_eq!(json["module"], "visgraph");
    assert_eq!(
        json["faceOrder"],
        serde_json::json!(["down", "up", "north", "south", "west", "east"])
    );

    let cases = json["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 6);
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let mut graph = VisGraph::new();
        for opaque in case["opaque"].as_array().unwrap() {
            let cell = opaque.as_array().unwrap();
            graph.set_opaque_local(
                cell[0].as_u64().unwrap() as usize,
                cell[1].as_u64().unwrap() as usize,
                cell[2].as_u64().unwrap() as usize,
            );
        }
        assert_eq!(
            graph.opaque_count(),
            case["opaqueCount"].as_u64().unwrap() as usize,
            "opaque count for {name}"
        );

        let visibility = graph.resolve();
        let rows = case["visibility"].as_array().unwrap();
        for first in SectionFace::ALL {
            let columns = rows[first.index()].as_array().unwrap();
            for second in SectionFace::ALL {
                assert_eq!(
                    visibility.visibility_between(first, second),
                    columns[second.index()].as_bool().unwrap(),
                    "visibility mismatch for case {name}, {first:?} -> {second:?}"
                );
            }
        }
    }
}
