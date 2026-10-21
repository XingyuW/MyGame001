
// These pin the current scheduled fixture contracts, not unsupported original-engine flight physics.
#[test]
fn ra9_missile_replacement_has_exact_ready_and_next_launch_boundaries() {
    let (mut app, launcher, truck) = launcher_and_truck();
    set_hp(&mut app, truck, 10_000);
    let events = first_rocket(&mut app, launcher, truck);
    assert_eq!(detonated(&events).len(), 1);
    // spawned_units installs Lost(200), then bays consumes this tick's first timer step.
    assert_eq!(state(&app).bays[&launcher].slots, [Slot::Lost(199)]);
    // End this fixture target after its real impact; idle acquisition must not queue another shot.
    set_hp(&mut app, truck, 0);
    let events = tick(&mut app, vec![issue(P0, sim::Command::Stop { units: vec![launcher] })]);
    assert!(rejections(&events).is_empty());
    assert_eq!(state(&app).bays[&launcher].slots, [Slot::Lost(198)]);
    for remaining in (1..198).rev() {
        let events = tick(&mut app, vec![]);
        assert_eq!(state(&app).bays[&launcher].slots, [Slot::Lost(remaining)]);
        assert!(launched(&events).is_empty());
    }
    let events = tick(&mut app, vec![]);
    assert_eq!(state(&app).bays[&launcher].slots, [Slot::Ready]);
    assert!(launched(&events).is_empty(), "ready alone is not a new attack");
    let next_target = spawn(&mut app, P1, "truck", 15, 10);
    let events = tick(&mut app, vec![attack(P0, launcher, next_target)]);
    assert_eq!(shots(&events, launcher).len(), 1);
    assert!(launched(&events).is_empty(), "Combat's launch shot is after Economy's bay pass");
    let events = tick(&mut app, vec![]);
    assert_eq!(launched(&events).len(), 1, "next Economy launches the ready missile");
    assert_cells(&mut app, "exact missile regeneration");
}

#[test]
fn ra9_returned_carrier_aircraft_rearms_at_the_exact_scheduled_boundary() {
    let (mut app, carrier, truck) = carrier_and_truck();
    set_hp(&mut app, truck, 10_000);
    let mut events = tick(&mut app, vec![attack(P0, carrier, truck)]);
    let mut first = None;
    for _ in 0..20 {
        if let Some((_, unit)) = launched(&events).first() { first = Some(*unit); break; }
        events = tick(&mut app, vec![]);
    }
    let first = first.expect("first actual launch");
    let events = tick(&mut app, vec![issue(P0, sim::Command::Stop { units: vec![carrier] })]);
    assert!(rejections(&events).is_empty());
    let mut came_back = false;
    for _ in 0..200 {
        let events = tick(&mut app, vec![]);
        if recovered(&events).contains(&(carrier, first)) { came_back = true; break; }
    }
    assert!(came_back, "real launched aircraft must fire and return");
    assert_eq!(state(&app).bays[&carrier].slots[0], Slot::Reloading(99));
    // End the durable fixture target after a real sortie; Stop alone permits later idle acquisition.
    set_hp(&mut app, truck, 0);
    let events = tick(&mut app, vec![issue(P0, sim::Command::Stop { units: vec![carrier] })]);
    assert!(rejections(&events).is_empty());
    assert_eq!(state(&app).bays[&carrier].slots[0], Slot::Reloading(98));
    for remaining in (1..98).rev() {
        let events = tick(&mut app, vec![]);
        assert_eq!(state(&app).bays[&carrier].slots[0], Slot::Reloading(remaining));
        assert!(launched(&events).is_empty());
    }
    tick(&mut app, vec![]);
    assert_eq!(state(&app).bays[&carrier].slots[0], Slot::Ready);
    assert_cells(&mut app, "exact carrier rearm");
}
