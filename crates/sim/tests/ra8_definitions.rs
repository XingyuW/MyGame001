//! RA8 immutable sharing preserves value semantics and independent consuming loaders.
use sim::{Definitions, load_ruleset};
use std::path::PathBuf;

#[test]
fn ra8_clones_share_read_only_tables_but_loading_a_variant_replaces_only_that_clone() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let original = load_ruleset(root.join("../../assets/data")).unwrap();
    let copy = original.clone();
    assert_eq!(original, copy);
    assert!(std::ptr::eq(original.units(), copy.units()));
    assert!(std::ptr::eq(original.arsenal(), copy.arsenal()));
    assert!(std::ptr::eq(original.weapons(), copy.weapons()));
    let variant = copy.with_economy(root.join("tests/fixtures/data/legacy/economy.ron"));
    // That economy names legacy units, so its typed rejection cannot alter the original.
    assert!(variant.is_err());
    assert_eq!(
        original,
        load_ruleset(root.join("../../assets/data")).unwrap()
    );
    let unit_file = root.join("../../assets/data/units.ron");
    let units = sim::load_unit_definitions(unit_file).unwrap();
    let empty_combat = Definitions::default();
    assert_ne!(units, empty_combat);
    let loaded = units
        .clone()
        .with_economy(root.join("../../assets/data/economy.ron"))
        .unwrap();
    assert_eq!(units.buildings().len(), 0);
    assert!(!loaded.buildings().is_empty());
}
