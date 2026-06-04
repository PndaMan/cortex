//! Vector helpers. Embeddings are stored as little-endian f32 BLOBs — which is
//! exactly `sqlite-vec`'s compact float32 format — so retrieval ranks in SQL via
//! `vec_distance_cosine` (see `repo::search_chunks`). The `cosine` helper here
//! remains as the tolerant Rust-scan fallback and for tests. (Decision: sqlite-vec
//! is the locked vector-search target — now in place.)

pub fn f32s_to_blob(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

pub fn blob_to_f32s(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_roundtrip() {
        let v = vec![0.1f32, -2.0, 3.5, 0.0];
        let b = f32s_to_blob(&v);
        assert_eq!(blob_to_f32s(&b), v);
    }

    #[test]
    fn cosine_identity_and_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((cosine(&a, &a) - 1.0).abs() < 1e-6);
        assert!(cosine(&a, &b).abs() < 1e-6);
    }
}
