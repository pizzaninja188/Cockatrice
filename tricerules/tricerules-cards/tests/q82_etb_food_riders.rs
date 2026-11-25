//! Registry conformance for the queue-v82 03-etb-food-riders cohort.
//!
//! Every card has an enters trigger that creates one Food token plus one printed rider.
//! Fingerprints are checked after the presentation metadata refresh.

use tricerules_cards::primitives::{
    AbilityCost, Amount, SpellEffectKind, TargetController, TargetKind,
};
use tricerules_cards::{
    registry, AbilityPresentation, AbilitySourceZone, CardFace, Keyword, Layout,
};

struct Case {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    keyword: Option<Keyword>,
    oracle_line: u16,
    fingerprint: &'static str,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "fierce_witchstalker",
            name: "Fierce Witchstalker",
            mana: "{2}{G}{G}",
            types: &["Creature", "Wolf"],
            power: 4,
            toughness: 4,
            keyword: Some(Keyword::Trample),
            oracle_line: 2,
            fingerprint: "dc198c8c8fc4c529b8786182cc30f7187242cc2f682bcafc027dc9703e5ccffe",
        },
        Case {
            id: "generous_ent",
            name: "Generous Ent",
            mana: "{5}{G}",
            types: &["Creature", "Treefolk"],
            power: 5,
            toughness: 7,
            keyword: Some(Keyword::Reach),
            oracle_line: 2,
            fingerprint: "1df0b3c393112209cc2b3d8dc3748e1158128a8ca7fc9f0801ea005d1e3885f7",
        },
        Case {
            id: "tempting_witch",
            name: "Tempting Witch",
            mana: "{2}{B}",
            types: &["Creature", "Human", "Warlock"],
            power: 1,
            toughness: 3,
            keyword: None,
            oracle_line: 1,
            fingerprint: "1ccad34f2930ab0e2f394fed5ce5263ff72a203cea4a5bfc71c9faffcb8d5bba",
        },
    ]
}

fn face(case: &Case) -> &'static CardFace {
    let definition = registry::global()
        .get(case.id)
        .unwrap_or_else(|| panic!("missing {}", case.id));
    assert_eq!(definition.layout, Layout::Normal, "{}", case.id);
    assert_eq!(
        registry::global().id_for_name(case.name),
        Some(case.id),
        "{}",
        case.id
    );
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), case.id, "{} face id", case.id);
    assert_eq!(face.name, case.name, "{} name", case.id);
    assert_eq!(face.mana_cost.to_string(), case.mana, "{} mana", case.id);
    assert_eq!(face.types, case.types, "{} types", case.id);
    assert_eq!(
        (face.power, face.toughness),
        (Some(case.power), Some(case.toughness)),
        "{} body",
        case.id
    );
    assert_eq!(
        face.keywords,
        case.keyword.into_iter().collect::<Vec<_>>(),
        "{} keywords",
        case.id
    );
    assert!(face.static_abilities.is_empty(), "{}", case.id);
    assert!(face.spell_effect.is_empty(), "{}", case.id);
    face
}

fn food_trigger<'a>(face: &'a CardFace, case: &Case) -> &'a tricerules_cards::TriggeredAbilityDef {
    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("{}: one entry trigger", case.id);
    };
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![case.oracle_line]),
        "{}",
        case.id
    );
    assert_eq!(
        trigger.trigger,
        tricerules_cards::TriggerCondition::WhenSelfEntersBattlefield,
        "{}",
        case.id
    );
    assert!(matches!(
        trigger.effect.as_slice(),
        [SpellEffectKind::CreateTokens { token, count: Amount::Fixed(1), .. }]
            if token == "food"
    ));
    assert!(trigger.targeting.is_none(), "{}", case.id);
    trigger
}

#[test]
fn q82_food_map_definitions() {
    for case in cases() {
        let f = face(&case);
        food_trigger(f, &case);
    }
}

#[test]
fn q82_food_generous_ent_has_forestcycling() {
    let case = &cases()[1];
    let f = face(case);
    let [cycling] = f.activated_abilities.as_slice() else {
        panic!("Generous Ent: one hand ability");
    };
    assert_eq!(cycling.ability_id.as_str(), "activated_01");
    assert_eq!(
        cycling.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(cycling.source_zone, AbilitySourceZone::Hand);
    assert_eq!(cycling.costs.len(), 2);
    assert!(matches!(cycling.costs[0], AbilityCost::Mana(ref mana) if mana.to_string() == "{1}"));
    assert_eq!(cycling.costs[1], AbilityCost::DiscardSelf);
    assert!(matches!(
        cycling.effect.as_slice(),
        [SpellEffectKind::SearchLibrary { filter: Some(filter), reveal: true, .. }]
            if filter.required_subtypes == ["Forest"]
    ));
    assert!(cycling.targeting.is_none());
}

#[test]
fn q82_food_tempting_witch_has_the_printed_food_sacrifice_ability() {
    let case = &cases()[2];
    let f = face(case);
    let [ability] = f.activated_abilities.as_slice() else {
        panic!("Tempting Witch: one activated ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
    assert!(matches!(ability.costs[0], AbilityCost::Mana(ref mana) if mana.to_string() == "{2}"));
    assert_eq!(ability.costs[1], AbilityCost::Tap);
    assert!(matches!(
        ability.costs[2],
        AbilityCost::SacrificePermanent { ref filter, count: 1 }
            if filter.kind == TargetKind::AnyPermanent
                && filter.controller == TargetController::You
                && filter.required_subtypes == ["Food"]
    ));
    assert!(matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::TargetPlayerLosesLife { amount: 3, target }]
            if target.kind == TargetKind::AnyPlayer
    ));
}

#[test]
fn q82_food_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for case in cases() {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", case.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", case.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], case.id);
        assert_eq!(fields[1], case.name);
        assert_eq!(fields[2], case.id, "face id drift for {}", case.id);
        assert_eq!(
            fields[4], case.fingerprint,
            "fingerprint drift for {}",
            case.id
        );
    }
}
