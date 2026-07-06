use super::*;

#[test]
fn generated_chunk_preserves_scheduled_ticks_from_mutable_buffer() {
    let mut buffer = MutableChunkBlockBuffer::new(2, -3, 0, 32);
    buffer.schedule_block_tick(33, 8, -47, "minecraft:stone", 2);
    buffer.schedule_liquid_tick(34, 9, -46, "minecraft:water", 0);

    let chunk = GeneratedChunk::from_mutable_buffer(buffer);

    assert_eq!(
        chunk.block_ticks(),
        &[ScheduledTick::new(33, 8, -47, "minecraft:stone", 2)]
    );
    assert_eq!(
        chunk.liquid_ticks(),
        &[ScheduledTick::new(34, 9, -46, "minecraft:water", 0)]
    );
}

#[test]
fn generated_chunk_converts_to_packed_snapshot() {
    let mut blocks = vec![AIR; 32 * 16 * 16];
    blocks[16 * 16 * 16] = STONE;
    let biomes = vec![6; mclone_core::expected_chunk_biome_count(32)];
    let chunk = GeneratedChunk::from_raw_parts_with_ticks_and_biomes(
        2,
        -3,
        0,
        32,
        blocks,
        biomes.clone(),
        Vec::new(),
        Vec::new(),
    );

    let snapshot = chunk.to_chunk_snapshot(
        mclone_core::ChunkRevision(9),
        mclone_core::ChunkStatus::Surface,
    );

    assert_eq!(snapshot.pos, mclone_core::ChunkPos::new(2, -3));
    assert_eq!(snapshot.revision, mclone_core::ChunkRevision(9));
    assert_eq!(snapshot.status, mclone_core::ChunkStatus::Surface);
    assert_eq!(snapshot.biomes, biomes);
    assert_eq!(snapshot.sections.len(), 1);
    assert_eq!(snapshot.sections[0].section_y, 1);
    assert_eq!(
        snapshot.sections[0].unpack_block_state_ids()[0],
        mclone_core::BlockStateId(STONE as u32)
    );
}
