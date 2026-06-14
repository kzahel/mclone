#![forbid(unsafe_code)]

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SectionMeshStats {
    pub vertex_count: u32,
    pub index_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_mesh_stats_are_zero() {
        assert_eq!(SectionMeshStats::default().vertex_count, 0);
        assert_eq!(SectionMeshStats::default().index_count, 0);
    }
}
