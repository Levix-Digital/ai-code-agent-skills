//! The launchers download the release named after `dev-cycle-protocol/bin/engine-version`;
//! it must be the version this workspace builds.

#[test]
fn engine_version_file_matches_the_crate_version() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dev-cycle-protocol/bin/engine-version"
    );
    let file = std::fs::read_to_string(path).expect("engine-version");
    assert_eq!(file.trim(), env!("CARGO_PKG_VERSION"));
}
