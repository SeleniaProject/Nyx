use nyx_fec::{NyxFec, DATA_SHARDS, PARITY_SHARDS, SHARD_SIZE};

#[test]
fn rs_fec_reconstruct_all_data_loss() {
    let codec = NyxFec::new();
    // create shards
    let mut shards: Vec<Vec<u8>> = (0..DATA_SHARDS)
        .map(|i| vec![i as u8; SHARD_SIZE])
        .collect();
    shards.extend((0..PARITY_SHARDS).map(|_| vec![0u8; SHARD_SIZE]));
    let mut mut_refs: Vec<&mut [u8]> = shards.iter_mut().map(|v| v.as_mut_slice()).collect();
    codec.encode(&mut mut_refs).unwrap();

    // simulate loss of all data shards
    let mut present: Vec<bool> = vec![false; DATA_SHARDS + PARITY_SHARDS];
    for i in DATA_SHARDS..DATA_SHARDS+PARITY_SHARDS { present[i] = true; }

    // zero out data shards
    for i in 0..DATA_SHARDS { mut_refs[i].fill(0); }

    // Attempt reconstruct: impossible (only 3 parity shards < required 10 total shards for RS MDS recovery)
    let res = codec.reconstruct(&mut mut_refs, &mut present);
    assert!(res.is_err(), "Reconstruction should fail when ALL data shards are lost with insufficient parity shards");
} 