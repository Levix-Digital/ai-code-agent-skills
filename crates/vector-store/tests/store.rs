//! The store through its client, with fixed embeddings (the embedding model is not needed).

use std::path::PathBuf;

use vector_store::{Client, Include, MetaValue, Metadata, Where};

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vector-store-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Deterministic unit vectors.
fn vectors(n: usize, dim: usize, seed: u64) -> Vec<Vec<f32>> {
    let mut state = seed;
    (0..n)
        .map(|_| {
            let v: Vec<f32> = (0..dim)
                .map(|_| {
                    state = state
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    ((state >> 33) as f32 / (1u64 << 31) as f32) - 0.5
                })
                .collect();
            let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            v.into_iter().map(|x| x / n).collect()
        })
        .collect()
}

fn meta(pairs: &[(&str, MetaValue)]) -> Metadata {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

fn ids(names: &[&str]) -> Vec<String> {
    names.iter().map(|s| s.to_string()).collect()
}

#[test]
fn upsert_get_filter_and_delete() {
    let dir = temp("crud");
    let mut c = Client::persistent(&dir).unwrap();
    let col = c.get_or_create_collection("plan_index", None).unwrap();
    let v = vectors(3, 8, 1);
    c.upsert_embeddings(
        &col,
        &ids(&["a", "b", "c"]),
        &v,
        &ids(&["doc a", "doc b", "doc c"]),
        &[
            meta(&[
                ("project", MetaValue::Str("p".into())),
                ("n", MetaValue::Int(1)),
            ]),
            meta(&[
                ("project", MetaValue::Str("q".into())),
                ("n", MetaValue::Int(2)),
            ]),
            meta(&[
                ("project", MetaValue::Str("p".into())),
                ("n", MetaValue::Float(3.5)),
            ]),
        ],
    )
    .unwrap();
    assert_eq!(c.count(&col).unwrap(), 3);
    let got = c
        .get(
            &col,
            None,
            Some(&Where::eq("project", MetaValue::Str("p".into()))),
            None,
            0,
            &[Include::Metadatas],
        )
        .unwrap();
    assert_eq!(got.ids, ids(&["a", "c"]));

    // An upsert keeps the keys it does not name.
    c.upsert_embeddings(
        &col,
        &ids(&["a"]),
        &v[..1],
        &ids(&["doc a2"]),
        &[meta(&[("n", MetaValue::Int(9))])],
    )
    .unwrap();
    let got = c
        .get(
            &col,
            Some(&ids(&["a"])),
            None,
            None,
            0,
            &[Include::Documents, Include::Metadatas],
        )
        .unwrap();
    assert_eq!(got.documents, vec![Some("doc a2".to_string())]);
    let m = got.metadatas[0].clone().unwrap();
    assert!(m.contains(&("project".to_string(), MetaValue::Str("p".into()))));
    assert!(m.contains(&("n".to_string(), MetaValue::Int(9))));

    c.delete(
        &col,
        None,
        Some(&Where::eq("project", MetaValue::Str("q".into()))),
    )
    .unwrap();
    assert_eq!(
        c.get(&col, None, None, None, 0, &[]).unwrap().ids,
        ids(&["a", "c"])
    );

    let hits = c.query_embedding(&col, v[2].clone(), 1, None).unwrap();
    assert_eq!(hits.ids, ids(&["c"]));
    assert!(hits.distances[0].abs() < 1e-5);

    let err = c.upsert_embeddings(
        &col,
        &ids(&["d"]),
        &vectors(1, 4, 2),
        &ids(&["d"]),
        &[Metadata::new()],
    );
    assert_eq!(
        err.unwrap_err().to_string(),
        "Collection expecting embedding with dimension of 8, got 4"
    );
    let err = c.upsert_embeddings(
        &col,
        &ids(&["d", "d"]),
        &vectors(2, 8, 2),
        &ids(&["d", "d"]),
        &[],
    );
    assert_eq!(err.unwrap_err().to_string(), "Expected IDs to be unique");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_index_persists_and_reopens() {
    let dir = temp("persist");
    let n = 300;
    let v = vectors(n, 16, 7);
    let names: Vec<String> = (0..n).map(|i| format!("id{i}")).collect();
    let docs: Vec<String> = names.iter().map(|i| format!("doc {i}")).collect();
    {
        let mut c = Client::persistent(&dir).unwrap();
        let col = c.get_or_create_collection("plan_index", None).unwrap();
        // Four rounds of 300 upserts: past the 1000 records that persist the index.
        for _ in 0..4 {
            c.upsert_embeddings(&col, &names, &v, &docs, &[]).unwrap();
        }
    }
    let segment = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.is_dir())
        .expect("vector segment folder");
    for f in [
        "header.bin",
        "data_level0.bin",
        "length.bin",
        "link_lists.bin",
        "index_metadata.pickle",
    ] {
        assert!(segment.join(f).is_file(), "{f}");
    }

    let mut c = Client::persistent(&dir).unwrap();
    let col = c.get_collection("plan_index").unwrap();
    assert_eq!(c.count(&col).unwrap(), n as u64);
    for i in [0, 17, 299] {
        let hits = c.query_embedding(&col, v[i].clone(), 3, None).unwrap();
        assert_eq!(hits.ids[0], names[i]);
        assert_eq!(hits.documents[0], Some(docs[i].clone()));
    }
    let only: Vec<String> = names[..5].to_vec();
    c.delete(&col, Some(&names[5..]), None).unwrap();
    let hits = c.query_embedding(&col, v[200].clone(), 10, None).unwrap();
    assert_eq!(hits.ids.len(), 5);
    assert!(hits.ids.iter().all(|i| only.contains(i)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_collection_name_is_validated() {
    let dir = temp("names");
    let mut c = Client::persistent(&dir).unwrap();
    assert!(c.get_or_create_collection("x", None).is_err());
    assert_eq!(
        c.get_collection("missing").unwrap_err().to_string(),
        "Collection [missing] does not exist"
    );
    // Opening again finds the migrations applied.
    drop(c);
    let mut c = Client::persistent(&dir).unwrap();
    let a = c.get_or_create_collection("plan_index", None).unwrap();
    let b = c.get_or_create_collection("plan_index", None).unwrap();
    assert_eq!(a.id, b.id);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn equal_distances_come_in_insertion_order() {
    let dir = temp("ties");
    let mut c = Client::persistent(&dir).unwrap();
    let col = c.get_or_create_collection("plan_index", None).unwrap();
    let v = vectors(1, 8, 3).remove(0);
    let names = ids(&["z", "a", "m"]);
    let same = vec![v.clone(), v.clone(), v.clone()];
    c.upsert_embeddings(&col, &names, &same, &ids(&["", "", ""]), &[])
        .unwrap();
    let hits = c.query_embedding(&col, v, 3, None).unwrap();
    assert_eq!(hits.ids, names);
    let _ = std::fs::remove_dir_all(&dir);
}
