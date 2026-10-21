//! UNCOMPILED / UNRUN: charge-origin hash property, not source-death runtime proof.
use super::*;
#[test]
fn ra9_charge_origin_owner_and_type_are_independently_hashed() {
    let hash = |owner, item| {
        let mut state = SpecialState::default();
        state.charges.insert(
            SimId(9),
            Charge {
                due: 100,
                damage: 400,
                source: SimId(1),
                c4: false,
                identity: crate::combat::DamageIdentity { owner, item },
            },
        );
        let mut h = Fnv1a64::new();
        super::hash(&state, &mut h);
        h.finish()
    };
    let base = hash(PlayerId(0), ProductionItem::Unit(crate::UnitKind(2)));
    assert_ne!(
        base,
        hash(PlayerId(1), ProductionItem::Unit(crate::UnitKind(2)))
    );
    assert_ne!(
        base,
        hash(PlayerId(0), ProductionItem::Unit(crate::UnitKind(3)))
    );
    assert_ne!(
        base,
        hash(
            PlayerId(0),
            ProductionItem::Building(crate::BuildingKind(2))
        )
    );
}
