//! Registry conformance for the queue-v54 batch C cohorts.
#![allow(clippy::type_complexity)]
use tricerules_cards::primitives::ActivatedAbilityDef;
use tricerules_cards::{registry, AbilityPresentation, CardFace, Layout};

fn identity(id: &str, name: &str, mana: &str, types: &[&str]) -> &'static CardFace {
    let definition = registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    let f = definition.primary_face();
    assert_eq!(f.name, name, "{id} name");
    assert_eq!(f.mana_cost.to_string(), mana, "{id} mana");
    assert_eq!(
        f.types.iter().map(String::as_str).collect::<Vec<_>>(),
        types,
        "{id} types"
    );
    f
}

fn assert_pt(f: &CardFace, id: &str, power: u32, toughness: u32) {
    assert_eq!(
        (f.power, f.toughness),
        (Some(power), Some(toughness)),
        "{id} P/T"
    );
}

fn activation<'a>(f: &'a CardFace, index: usize, id: &str) -> &'a ActivatedAbilityDef {
    f.activated_abilities
        .get(index)
        .unwrap_or_else(|| panic!("{id}: missing activated ability {index}"))
}

#[test]
fn q54_equipment_pump_cards_map_definitions() {
    let f = identity(
        "darksteel_axe",
        "Darksteel Axe",
        "{1}",
        &["Artifact", "Equipment"],
    );
    assert_eq!(format!("{:?}", f.keywords), "[Indestructible]");
    let modifier = format!("{:?}", f.static_abilities[0].definition);
    assert!(modifier.contains("AttachedModifier"));
    assert!(modifier.contains("delta_power: 2"), "{modifier}");
    assert!(modifier.contains("delta_toughness: 0"), "{modifier}");
    assert!(modifier.contains("keywords: []"), "{modifier}");
    assert_eq!(
        f.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    let equip = activation(f, 0, "darksteel_axe");
    assert_eq!(
        equip.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert!(format!("{:?}", equip.costs).contains("Generic(2)"));

    let f = identity("no-dachi", "No-Dachi", "{2}", &["Artifact", "Equipment"]);
    let modifier = format!("{:?}", f.static_abilities[0].definition);
    assert!(modifier.contains("AttachedModifier"));
    assert!(modifier.contains("delta_power: 2"), "{modifier}");
    assert!(modifier.contains("keywords: [FirstStrike]"), "{modifier}");
    assert_eq!(
        f.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    let equip = activation(f, 0, "no-dachi");
    assert_eq!(
        equip.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(format!("{:?}", equip.costs).contains("Generic(3)"));
    assert!(
        format!("{:?}", equip.effect).contains("controller: You"),
        "No-Dachi equips only a creature you control"
    );

    let f = identity(
        "prying_blade",
        "Prying Blade",
        "{1}",
        &["Artifact", "Equipment"],
    );
    let modifier = format!("{:?}", f.static_abilities[0].definition);
    assert!(modifier.contains("AttachedModifier"));
    assert!(modifier.contains("delta_power: 1"), "{modifier}");
    let trigger = f
        .triggered_abilities
        .first()
        .expect("Prying Blade damage trigger");
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(format!("{:?}", trigger.trigger)
        .contains("WheneverAttachedObjectDealsCombatDamageToPlayer"));
    assert!(format!("{:?}", trigger.effect).contains("treasure"));
    let equip = activation(f, 0, "prying_blade");
    assert_eq!(
        equip.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert!(
        format!("{:?}", equip.effect).contains("controller: You"),
        "Prying Blade equips only a creature you control"
    );
}

#[test]
fn q54_pump_spell_riders_map_definitions() {
    let f = identity("iron_will", "Iron Will", "{W}", &["Instant"]);
    assert!(format!("{:?}", f.spell_effect).contains("power: 0, toughness: 4"));
    let cycle = activation(f, 0, "iron_will");
    assert_eq!(
        cycle.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    let costs = format!("{:?}", cycle.costs);
    assert!(costs.contains("Generic(2)"), "{costs}");
    assert!(costs.contains("DiscardSelf"), "{costs}");
    assert!(format!("{:?}", cycle.effect).contains("Draw"));

    let f = identity(
        "sokkas_sword_training",
        "Sokka's Sword Training",
        "{1}{W}",
        &["Instant", "Lesson"],
    );
    let debug = format!("{:?}", f.spell_effect);
    assert!(debug.contains("power: 2, toughness: 2"), "{debug}");
    assert!(debug.contains("CreateTokens"), "{debug}");
    assert!(debug.contains("clue"), "{debug}");

    let f = identity(
        "spatial_contortion",
        "Spatial Contortion",
        "{1}{C}",
        &["Instant"],
    );
    assert!(format!("{:?}", f.spell_effect).contains("power: 3, toughness: -3"));
}

#[test]
fn q54_each_player_sacrifices_cards_map_definitions() {
    let f = identity("innocent_blood", "Innocent Blood", "{B}", &["Sorcery"]);
    let debug = format!("{:?}", f.spell_effect);
    assert!(debug.contains("ChooseAndSacrifice"), "{debug}");
    assert!(debug.contains("EachPlayer"), "{debug}");

    let f = identity(
        "abyssal_gatekeeper",
        "Abyssal Gatekeeper",
        "{1}{B}",
        &["Creature", "Horror"],
    );
    assert_pt(f, "abyssal_gatekeeper", 1, 1);
    let trigger = f
        .triggered_abilities
        .first()
        .expect("Abyssal Gatekeeper dies trigger");
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(format!("{:?}", trigger.trigger).contains("WhenSelfDies"));
    assert!(format!("{:?}", trigger.effect).contains("EachPlayer"));

    let f = identity(
        "demons_disciple",
        "Demon's Disciple",
        "{2}{B}",
        &["Creature", "Human", "Cleric"],
    );
    assert_pt(f, "demons_disciple", 3, 1);
    let trigger = f
        .triggered_abilities
        .first()
        .expect("Demon's Disciple entry trigger");
    assert!(format!("{:?}", trigger.trigger).contains("WhenSelfEntersBattlefield"));
    let effect = format!("{:?}", trigger.effect);
    assert!(effect.contains("ChooseAndSacrifice"), "{effect}");
    assert!(effect.contains("Planeswalker"), "{effect}");
}

#[test]
fn q54_c_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, face, digest) in [
        (
            "abyssal_gatekeeper",
            "Abyssal Gatekeeper",
            "abyssal_gatekeeper",
            "2baec0ddae181430a8aca47b044f3885c6e7795207bb04f87fae004e8020a0e4",
        ),
        (
            "darksteel_axe",
            "Darksteel Axe",
            "darksteel_axe",
            "1c60bc135e9dc393ebf8ad3c4c13c7f7aeb72af437eae5fead48a2c218c9df8f",
        ),
        (
            "demons_disciple",
            "Demon's Disciple",
            "demons_disciple",
            "b8396e5c15de6dcd21514bc27d2436e34ea948d20065a6228d3159968f81ea46",
        ),
        (
            "innocent_blood",
            "Innocent Blood",
            "innocent_blood",
            "dacdd79c86ba78dc35bde77e451fc836c39d819eabcffbe0c6b4694a5a1598f6",
        ),
        (
            "iron_will",
            "Iron Will",
            "iron_will",
            "d9b92e33f0f0ab4a132c2427749635049ff8e92e4f1af49f64a962926f7773bd",
        ),
        (
            "no-dachi",
            "No-Dachi",
            "no_dachi",
            "4a826048f3617436f5f54e906f8a38dcfb7624c1c607cc1497b1003aef4d6b4a",
        ),
        (
            "prying_blade",
            "Prying Blade",
            "prying_blade",
            "39bf3f73ab55ebd4dc3ad47b0057ca611a3190de3bd2994c894e5e6cacd74d7f",
        ),
        (
            "sokkas_sword_training",
            "Sokka's Sword Training",
            "sokkas_sword_training",
            "227a5ec14d01e6e8a63e8b87fb8bcd79b0885dbe9b3f9ce54656d4726fdc8cfe",
        ),
        (
            "spatial_contortion",
            "Spatial Contortion",
            "spatial_contortion",
            "4d8c9e5f43cea6c17404db7406d391c43b827f3b29174ff6896aed5799a9b273",
        ),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "{id}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], face, "face id drift for {id}");
        assert_eq!(fields[3], name);
        assert_eq!(fields[4], digest, "pinned fingerprint digest for {id}");
    }
}
