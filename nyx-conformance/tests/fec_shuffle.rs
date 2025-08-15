use nyx_fec::{NyxFec, DATA_SHARDS, PARITY_SHARDS, SHARD_SIZE};

#[test]
fn fec_shuffle_verify() {
    let codec = NyxFec::new();
    let mut shards: Vec<Vec<u8>> = (0..DATA_SHARDS)
        .map(|i| vec![i as u8; SHARD_SIZE])
        .collect();
    shards.extend((0..PARITY_SHARDS).map(|_| vec![0u8; SHARD_SIZE]));
    let mut refs: Vec<&mut [u8]> = shards.iter_mut().map(|s| s.as_mut_slice()).collect();
    codec.encode(&mut refs).unwrap();
    // shuffle one data shard with first parity shard (layout disturbed)
    refs.swap(0, DATA_SHARDS);
    // Reconstruct slice array in canonical order (data shards first, then parity)
    // by mapping back using original indices.
    let mut canonical: Vec<&[u8]> = Vec::with_capacity(DATA_SHARDS + PARITY_SHARDS);
    // Data region: after swap, original data[0] moved to position DATA_SHARDS.
    for i in 0..DATA_SHARDS {
        // If i==0 fetch from swapped position, else from i
        if i == 0 {
            canonical.push(refs[DATA_SHARDS]);
        } else {
            canonical.push(refs[i]);
        }
    }
    // Parity region: first parity now at index 0 after swap
    canonical.push(refs[0]);
    for j in 1..PARITY_SHARDS {
        canonical.push(refs[DATA_SHARDS + j]);
    }
    assert!(codec.verify(&canonical).unwrap());
}
