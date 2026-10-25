//! RA7 GuardRange is required, bounded, audited independently and changes the replay hash.
use std::path::PathBuf;
fn base() -> sim::Definitions {
    sim::load_ruleset(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data")).unwrap()
}
fn variant(to: &str) -> Result<sim::Definitions, sim::DataError> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data");
    let text = std::fs::read_to_string(root.join("arsenal.ron")).unwrap();
    let from = "guard_range: 0,";
    assert!(text.contains(from));
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ra7_definitions");
    std::fs::create_dir_all(&dir).unwrap();
    let mut h = sim::Fnv1a64::new();
    h.write(to.as_bytes());
    let path = dir.join(format!("{}-{:x}.ron", std::process::id(), h.finish()));
    std::fs::write(&path, text.replacen(from, to, 1)).unwrap();
    base().with_arsenal(path)
}
fn hash(defs: sim::Definitions) -> u64 {
    let mut app = bevy_app::App::new();
    app.add_plugins(sim::SimPlugin { seed: 1 });
    app.world_mut().insert_resource(defs);
    sim::state_hash(app.world_mut())
}
#[test]
fn ra7_guard_range_is_required_bounded_and_hashed() {
    assert!(matches!(
        variant("").unwrap_err().kind,
        sim::DataErrorKind::MissingField
    ));
    for n in [-1, 65] {
        assert!(matches!(
            variant(&format!("guard_range: {n},")).unwrap_err().kind,
            sim::DataErrorKind::InvalidValue { .. }
        ));
    }
    assert!(variant("guard_range: 64,").is_ok());
    assert_ne!(hash(base()), hash(variant("guard_range: 1,").unwrap()));
}
