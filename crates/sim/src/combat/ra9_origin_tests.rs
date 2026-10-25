//! UNCOMPILED / UNRUN: helper/hash properties, not native impact proof.
use super::*;
#[test]
fn ra9_projectile_source_item_is_hashed_separately_from_credit_and_launch_owner() {
    let shell = Projectile {
        owner: PlayerId(0),
        attacker: SimId(1),
        source_item: Some(crate::ProductionItem::Unit(crate::UnitKind(2))),
        target: SimId(4),
        weapon: WeaponKind(0),
        origin: SubPos::new(0, 0),
        aim: SubPos::new(256, 0),
        scattered: false,
        air: false,
        flight: 5,
        remaining: 3,
        firepower_pm: 1000,
    };
    let hash = |p| {
        let mut h = Fnv1a64::new();
        Projectiles { list: vec![p] }.hash_into(&mut h);
        h.finish()
    };
    for source_item in [
        None,
        Some(crate::ProductionItem::Unit(crate::UnitKind(3))),
        Some(crate::ProductionItem::Building(crate::BuildingKind(2))),
    ] {
        assert_ne!(
            hash(shell),
            hash(Projectile {
                source_item,
                ..shell
            })
        );
    }
    let mut projectiles = Projectiles { list: vec![shell] };
    projectiles.reassign(SimId(1), SimId(9));
    let p = projectiles.iter().next().expect("projectile");
    assert_eq!(p.attacker, SimId(9));
    assert_eq!(p.owner, PlayerId(0));
    assert_eq!(p.source_item, shell.source_item);
}
