//! Registry conformance for the queue-v56 batch B cohorts.
use tricerules_cards::primitives::{
    AbilityCost, SpellEffectKind, TargetController, TargetKind, TriggerCondition,
};
use tricerules_cards::{registry, AbilityPresentation, CardFace, Layout, ManaCost};

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

#[test]
fn q56_b_shrink_rider_map_definitions() {
    for (id, name, mana, types, penalty, rider) in [
        (
            "bleed_dry",
            "Bleed Dry",
            "{2}{B}{B}",
            &["Instant"][..],
            13i32,
            "ExileIfWouldDieThisTurn",
        ),
        (
            "ob_nixiliss_cruelty",
            "Ob Nixilis's Cruelty",
            "{2}{B}",
            &["Instant"][..],
            5,
            "ExileIfWouldDieThisTurn",
        ),
        (
            "drown_in_ichor",
            "Drown in Ichor",
            "{1}{B}",
            &["Sorcery"][..],
            4,
            "Proliferate",
        ),
        (
            "whisper_of_the_dross",
            "Whisper of the Dross",
            "{B}",
            &["Instant"][..],
            1,
            "Proliferate",
        ),
    ] {
        let f = identity(id, name, mana, types);
        let effects = format!("{:?}", f.spell_effect);
        assert!(
            effects.contains(&format!("power: -{penalty}, toughness: -{penalty}")),
            "{id}: {effects}"
        );
        assert!(effects.contains(rider), "{id}: {effects}");
        let targeting = f.targeting.as_ref().expect("targeting");
        assert_eq!(targeting.groups.len(), 1, "{id}");
        assert_eq!(
            (targeting.groups[0].min, targeting.groups[0].max),
            (1, 1),
            "{id}"
        );
        if rider == "ExileIfWouldDieThisTurn" {
            assert_eq!(f.spell_effect.len(), 2, "{id}");
            assert_eq!(targeting.groups[0].effect_indices, vec![0, 1], "{id}");
        } else {
            assert_eq!(f.spell_effect.len(), 2, "{id}");
            assert_eq!(targeting.groups[0].effect_indices, vec![0], "{id}");
        }
    }
}

#[test]
fn q56_b_equipment_etb_map_definitions() {
    for (id, name, mana, equip, power, toughness, flying) in [
        (
            "bramble_armor",
            "Bramble Armor",
            "{1}{G}",
            "{4}",
            2i32,
            1i32,
            false,
        ),
        ("utility_knife", "Utility Knife", "{1}", "{3}", 1, 1, false),
        (
            "cliffhaven_kitesail",
            "Cliffhaven Kitesail",
            "{1}",
            "{2}",
            0,
            0,
            true,
        ),
        (
            "mechanical_glider",
            "Mechanical Glider",
            "{1}",
            "{2}",
            0,
            0,
            true,
        ),
    ] {
        let f = identity(id, name, mana, &["Artifact", "Equipment"]);
        let activated = &f.activated_abilities[0];
        assert_eq!(
            activated.presentation,
            AbilityPresentation::OracleLines(vec![3]),
            "{id}"
        );
        assert_eq!(
            activated.costs,
            [AbilityCost::Mana(
                ManaCost::parse(equip).expect("equip cost")
            )],
            "{id}: exact printed equip cost"
        );
        assert!(
            matches!(activated.effect.as_slice(), [SpellEffectKind::Equip { .. }]),
            "{id}: equip effect"
        );

        let triggered = &f.triggered_abilities[0];
        assert_eq!(
            triggered.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert_eq!(
            triggered.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        assert!(
            matches!(
                triggered.effect.as_slice(),
                [SpellEffectKind::AttachSource { target }]
                    if target.kind == TargetKind::Creature
                        && target.controller == TargetController::You
            ),
            "{id}: attach to a creature you control"
        );
        let targeting = triggered.targeting.as_ref().expect("entry targeting");
        assert_eq!(
            (targeting.groups[0].min, targeting.groups[0].max),
            (1, 1),
            "{id}"
        );

        let static_ability = &f.static_abilities[0];
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id}"
        );
        let definition = format!("{:?}", static_ability.definition);
        assert!(
            definition.contains("AttachedModifier"),
            "{id}: {definition}"
        );
        assert!(
            definition.contains(&format!("delta_power: {power}")),
            "{id}: {definition}"
        );
        assert!(
            definition.contains(&format!("delta_toughness: {toughness}")),
            "{id}: {definition}"
        );
        assert_eq!(definition.contains("Flying"), flying, "{id}: {definition}");
    }
}

#[test]
fn q56_b_flashback_token_map_definitions() {
    for (id, name, mana, flashback, types, token, count) in [
        (
            "call_of_the_herd",
            "Call of the Herd",
            "{2}{G}",
            "{3}{G}",
            &["Sorcery"][..],
            "elephant_g_3_3",
            1,
        ),
        (
            "elephant_ambush",
            "Elephant Ambush",
            "{2}{G}{G}",
            "{6}{G}{G}",
            &["Instant"][..],
            "elephant_g_3_3",
            1,
        ),
        (
            "lingering_souls",
            "Lingering Souls",
            "{2}{W}",
            "{1}{B}",
            &["Sorcery"][..],
            "spirit_w_1_1_flying",
            2,
        ),
    ] {
        let f = identity(id, name, mana, types);
        assert_eq!(
            f.flashback_cost.clone().map(|cost| cost.to_string()),
            Some(flashback.into()),
            "{id}"
        );
        let effects = format!("{:?}", f.spell_effect);
        assert!(effects.contains("CreateTokens"), "{id}: {effects}");
        assert!(effects.contains(token), "{id}: {effects}");
        assert!(
            effects.contains(&format!("count: Fixed({count})")),
            "{id}: {effects}"
        );
        assert!(
            f.targeting.is_none(),
            "{id}: token creation does not target"
        );
        assert!(
            registry::global().is_token(token),
            "{token} must be a registered token"
        );
    }
}

#[test]
fn q56_b_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, face, digest) in [
        (
            "bleed_dry",
            "Bleed Dry",
            "bleed_dry",
            "856ac33f4ed47aae63f590c06873a0171128a4ebad2ba09c8b843d6beb1fd2a5",
        ),
        (
            "ob_nixiliss_cruelty",
            "Ob Nixilis's Cruelty",
            "ob_nixiliss_cruelty",
            "478ebde2a981c80ef36828fcf4806ef9b002c57ad8b9c5a9d033df3a93319302",
        ),
        (
            "drown_in_ichor",
            "Drown in Ichor",
            "drown_in_ichor",
            "0ec4d9575b6474bfb16e9375c8d6f7e21f4d6cf14a5760c1e4d7f131ee593b4d",
        ),
        (
            "whisper_of_the_dross",
            "Whisper of the Dross",
            "whisper_of_the_dross",
            "ee40ce01afcd46ddc71447ad73437162c078936af1a1ffca108839b0ca47289f",
        ),
        (
            "bramble_armor",
            "Bramble Armor",
            "bramble_armor",
            "f69e89e6ab455acf7d9e17b685fa0b295ce3040a261107fd7bd64ca0e0df5d85",
        ),
        (
            "utility_knife",
            "Utility Knife",
            "utility_knife",
            "192a25ebcb89ffed368300b6f354bce6a48a7b133c24e40c938ecdc299a57b25",
        ),
        (
            "cliffhaven_kitesail",
            "Cliffhaven Kitesail",
            "cliffhaven_kitesail",
            "29e32475663c9a02605bc73e832e3baf2c06e2c55c5f8c9f4541601241922cae",
        ),
        (
            "mechanical_glider",
            "Mechanical Glider",
            "mechanical_glider",
            "e7961c75e94af77522eae09b3e0e6fdfcaa44b26b6f9e4d632dda63fbd4636c5",
        ),
        (
            "call_of_the_herd",
            "Call of the Herd",
            "call_of_the_herd",
            "a6575cb4a74363708cb629b98a4652bfb4911e7fc1c6505a08e9a3501b189324",
        ),
        (
            "elephant_ambush",
            "Elephant Ambush",
            "elephant_ambush",
            "917f04d799df480087f303edc2c11e35f183fa1f73240f5ede7e47b21709500b",
        ),
        (
            "lingering_souls",
            "Lingering Souls",
            "lingering_souls",
            "af45e3d9a3bec86399ba8a1cbe70e017881fb0c70721ac52d28dbe5d1a0c9721",
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
