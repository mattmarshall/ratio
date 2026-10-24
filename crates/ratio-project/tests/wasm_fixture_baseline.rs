fn main() {
    let root = std::path::PathBuf::from(std::env::var_os("TEST_TMPDIR").unwrap())
        .join("wasm-native-fixture");
    let local = backend_fixture::exercise(&root, None);
    let objects = std::sync::Arc::new(ratio_store::MemoryStore::new());
    let no_cache = backend_fixture::exercise_with(|| {
        ratio_store::FileBook::on_store("native", objects.clone()).unwrap()
    });
    assert_eq!(local, no_cache);
    println!("{}", serde_json::to_string(&local).unwrap());
}
