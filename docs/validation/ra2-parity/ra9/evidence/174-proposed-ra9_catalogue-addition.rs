
fn setup_catalogue(root: &std::path::Path, d: &Definitions, out: &mut String) -> Result<(), Box<dyn Error>> {
    let data = root.join("assets/data");
    let skirmish = sim::load_skirmish(data.join("skirmish.ron"), d)?;
    let ai = sim::load_ai(data.join("ai.ron"), d)?;
    sim::check_ai_factions(data.join("ai.ron"), &ai, &skirmish, d)?;
    writeln!(out, "\n## Actual loader counts\n\nUnits: {}; buildings: {}; weapons: {}; warheads: {}; armour classes: {}; skirmish countries: {}; maps: {}; raw AI plans: {}; difficulties: {}.\n\nRaw plans can include compatibility plans; country plans below use the actual plan_for selection and filtering. Map sizes and starts are the filed loader values; MAP GROWTH OFF limits players to the filed starts and MAP GROWTH ON derives starts for up to ten. This catalogue does not execute matches or certify yard exits.",
        d.units().len(), d.buildings().len(), d.weapons().len(), d.warheads().len(),
        d.armour_classes().len(), skirmish.factions.len(), skirmish.maps.len(), ai.plans.len(), ai.difficulties.len())?;
    out.push_str("\n## Filed maps\n\n| Map ID | Width | Height | Filed start positions | Maximum players without growth |\n|---|---:|---:|---:|---:|\n");
    for spec in &skirmish.maps {
        let map = sim::load_map(root.join("assets").join(sim::map_asset_path(&spec.id)))?;
        writeln!(out, "| {} | {} | {} | {} | {} |", spec.id, map.grid.width(), map.grid.height(),
            spec.starts.len(), spec.starts.len().min(sim::MAX_MATCH_PLAYERS))?;
    }
    let buildings = |values: &[sim::BuildingKind]| -> Result<String, Box<dyn Error>> {
        values.iter().map(|kind| d.building(*kind).map(|value| value.id.clone()).ok_or_else(|| "catalogue plan building identity".into()))
            .collect::<Result<Vec<_>, Box<dyn Error>>>().map(|values| values.join(", "))
    };
    let units = |values: &[sim::UnitKind]| -> Result<String, Box<dyn Error>> {
        values.iter().map(|kind| d.unit(*kind).map(|value| value.id.clone()).ok_or_else(|| "catalogue plan unit identity".into()))
            .collect::<Result<Vec<_>, Box<dyn Error>>>().map(|values| values.join(", "))
    };
    out.push_str("\n## Raw AI plan catalogue\n\n| Index | Side | Country ID | Power | Harvester | Build order | Army order | Defences | Support |\n|---|---|---|---|---|---|---|---|---|\n");
    for (index, plan) in ai.plans.iter().enumerate() {
        writeln!(out, "| {index} | {:?} | {:?} | {} | {} | {} | {} | {} | {} |", plan.side, plan.country,
            buildings(&[plan.power])?, units(&[plan.harvester])?, buildings(&plan.build_order)?,
            units(&plan.army)?, buildings(&plan.defences)?, units(&plan.support)?)?;
    }
    out.push_str("\n## Actual resolved country plans\n\n| Country | Side | Derived roster size | Power | Harvester | Build order | Army order | Tier reserves | Advanced tech level |\n|---|---|---:|---|---|---|---|---|---:|\n");
    for faction in &skirmish.factions {
        let plan = ai.plan_for(faction).ok_or("catalogue resolved country plan")?;
        writeln!(out, "| {} | {:?} | {} | {} | {} | {} | {} | {:?} | {} |", faction.id, faction.side,
            faction.roster.len(), buildings(&[plan.power])?, units(&[plan.harvester])?,
            buildings(&plan.build_order)?, units(&plan.army)?, plan.tier_reserves, plan.advanced_tech_level)?;
    }
    out.push_str("\n## Actual AI difficulties\n\n| ID | Think interval | Max army | Wave size | First wave | Wave patience | Defence radius |\n|---|---:|---:|---:|---:|---:|---:|\n");
    for difficulty in &ai.difficulties {
        writeln!(out, "| {} | {} | {} | {} | {} | {} | {} |", difficulty.id, difficulty.think_interval,
            difficulty.max_army, difficulty.wave_size, difficulty.first_wave, difficulty.wave_patience,
            difficulty.defence_radius)?;
    }
    Ok(())
}
