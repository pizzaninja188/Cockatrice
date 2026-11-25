//! Registry conformance for the queue-v65 batch A cohorts.
//!
//! Three stable typed shapes: an activated damage ability, an entry +1/+1-counter placement, and
//! destroy-target-permanent. Fingerprints are asserted once the presentation metadata is refreshed
//! from the pinned Oracle text.

use tricerules_cards::primitives::{
    AbilityCost, Amount, CounterKind, EffectSubject, Keyword, PermanentTypeFilter, SpellEffectKind,
    TargetController, TargetKind, TargetObjectExclusion, TriggerCondition,
};
use tricerules_cards::{
    registry, AbilityPresentation, ActivatedAbilityDef, CardFace, Layout, TriggeredAbilityDef,
};

fn face(id: &str, name: &str, mana: &str, types: &[&str]) -> &'static CardFace {
    let definition = registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(registry::global().id_for_name(name), Some(id), "{id}");
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

fn only_activated<'a>(f: &'a CardFace, id: &str) -> &'a ActivatedAbilityDef {
    let [ability] = f.activated_abilities.as_slice() else {
        panic!("{id}: one activated ability");
    };
    ability
}

fn only_triggered<'a>(f: &'a CardFace, id: &str) -> &'a TriggeredAbilityDef {
    let [ability] = f.triggered_abilities.as_slice() else {
        panic!("{id}: one triggered ability");
    };
    ability
}

#[test]
fn q65_activated_damage_map_definitions() {
    // Fireslinger: {T}: 1 damage to any target and 1 damage to you.
    let f = face(
        "fireslinger",
        "Fireslinger",
        "{1}{R}",
        &["Creature", "Human", "Wizard"],
    );
    assert_eq!((f.power, f.toughness), (Some(1), Some(1)));
    let ability = only_activated(f, "fireslinger");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(ability.costs.as_slice(), [AbilityCost::Tap]);
    assert!(matches!(
        ability.effect.as_slice(),
        [
            SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(1),
                target,
            },
            SpellEffectKind::DamagePlayer {
                amount: Amount::Fixed(1),
                ..
            },
        ] if target.kind == TargetKind::AnyTarget && target.any_of.is_none()
    ));
    let group = &ability.targeting.as_ref().unwrap().groups[0];
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.effect_indices, [0]);

    // Goblin Fireslinger: {T}: 1 damage to target player or planeswalker.
    let f = face(
        "goblin_fireslinger",
        "Goblin Fireslinger",
        "{R}",
        &["Creature", "Goblin", "Warrior"],
    );
    assert_eq!((f.power, f.toughness), (Some(1), Some(1)));
    let ability = only_activated(f, "goblin_fireslinger");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(ability.costs.as_slice(), [AbilityCost::Tap]);
    assert!(matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::DamageTarget {
            amount: Amount::Fixed(1),
            target,
        }] if target.any_of.as_ref().is_some_and(|branches| {
            branches.len() == 2
                && branches.iter().any(|leaf| leaf.kind == TargetKind::AnyPlayer)
                && branches.iter().any(|leaf| {
                    leaf.kind == TargetKind::AnyPermanent
                        && leaf.permanent_types.contains(&PermanentTypeFilter::Planeswalker)
                })
        })
    ));

    // Valakut Invoker: {8}: 3 damage to any target.
    let f = face(
        "valakut_invoker",
        "Valakut Invoker",
        "{2}{R}",
        &["Creature", "Human", "Shaman"],
    );
    assert_eq!((f.power, f.toughness), (Some(2), Some(3)));
    let ability = only_activated(f, "valakut_invoker");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(matches!(
        ability.costs.as_slice(),
        [AbilityCost::Mana(cost)] if cost.to_string() == "{8}"
    ));
    assert!(matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::DamageTarget {
            amount: Amount::Fixed(3),
            target,
        }] if target.kind == TargetKind::AnyTarget
    ));

    // Aeolipile: {1}, {T}, sacrifice: 2 damage to any target.
    let f = face("aeolipile", "Aeolipile", "{2}", &["Artifact"]);
    let ability = only_activated(f, "aeolipile");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(matches!(
        ability.costs.as_slice(),
        [AbilityCost::Mana(cost), AbilityCost::Tap, AbilityCost::SacrificeSelf]
            if cost.to_string() == "{1}"
    ));
    assert!(matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::DamageTarget {
            amount: Amount::Fixed(2),
            target,
        }] if target.kind == TargetKind::AnyTarget
    ));
}

#[test]
fn q65_etb_counter_map_definitions() {
    struct Case {
        id: &'static str,
        name: &'static str,
        mana: &'static str,
        types: &'static [&'static str],
        power: u32,
        toughness: u32,
        keywords: &'static [Keyword],
        line: u16,
        min: u32,
        max: u32,
        controller: TargetController,
        excludes_source: bool,
    }
    let cases = [
        Case {
            id: "gavony_silversmith",
            name: "Gavony Silversmith",
            mana: "{3}{W}",
            types: &["Creature", "Human", "Soldier"],
            power: 2,
            toughness: 3,
            keywords: &[],
            line: 1,
            min: 0,
            max: 2,
            controller: TargetController::Any,
            excludes_source: false,
        },
        Case {
            id: "basris_acolyte",
            name: "Basri's Acolyte",
            mana: "{2}{W}{W}",
            types: &["Creature", "Cat", "Cleric"],
            power: 2,
            toughness: 3,
            keywords: &[Keyword::Lifelink],
            line: 2,
            min: 0,
            max: 2,
            controller: TargetController::You,
            excludes_source: true,
        },
        Case {
            id: "keen-eyed_raven",
            name: "Keen-Eyed Raven",
            mana: "{2}{W}",
            types: &["Creature", "Bird"],
            power: 2,
            toughness: 2,
            keywords: &[Keyword::Flying],
            line: 2,
            min: 1,
            max: 1,
            controller: TargetController::You,
            excludes_source: true,
        },
        Case {
            id: "supply-line_cranes",
            name: "Supply-Line Cranes",
            mana: "{3}{W}{W}",
            types: &["Creature", "Bird"],
            power: 2,
            toughness: 4,
            keywords: &[Keyword::Flying],
            line: 2,
            min: 1,
            max: 1,
            controller: TargetController::Any,
            excludes_source: false,
        },
    ];
    for case in cases {
        let f = face(case.id, case.name, case.mana, case.types);
        assert_eq!(
            (f.power, f.toughness),
            (Some(case.power), Some(case.toughness))
        );
        assert_eq!(f.keywords.as_slice(), case.keywords, "{}", case.id);
        let ability = only_triggered(f, case.id);
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![case.line]),
            "{}",
            case.id
        );
        assert_eq!(
            ability.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{}",
            case.id
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::PutCounters {
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::Fixed(1),
                    subject: EffectSubject::Chosen(filter),
                }] if filter.kind == TargetKind::Creature
                    && filter.permanent_types.is_empty()
                    && filter.controller == case.controller
                    && filter.excluded_objects.contains(&TargetObjectExclusion::Source)
                        == case.excludes_source
            ),
            "{}: {:?}",
            case.id,
            ability.effect
        );
        let group = &ability.targeting.as_ref().unwrap().groups[0];
        assert_eq!((group.min, group.max), (case.min, case.max), "{}", case.id);
    }
}

#[test]
fn q65_destroy_permanent_map_definitions() {
    for (id, name, mana) in [
        ("vindicate", "Vindicate", "{1}{W}{B}"),
        ("desert_twister", "Desert Twister", "{4}{G}{G}"),
    ] {
        let f = face(id, name, mana, &["Sorcery"]);
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(matches!(
            f.spell_effect.as_slice(),
            [SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(filter),
            }] if filter.kind == TargetKind::AnyPermanent
                && filter.permanent_types.is_empty()
                && filter.controller == TargetController::Any
        ));
        let group = &f.targeting.as_ref().unwrap().groups[0];
        assert_eq!((group.min, group.max), (1, 1), "{id}");
    }

    let f = face(
        "universal_solvent",
        "Universal Solvent",
        "{1}",
        &["Artifact"],
    );
    let ability = only_activated(f, "universal_solvent");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(matches!(
        ability.costs.as_slice(),
        [AbilityCost::Mana(cost), AbilityCost::Tap, AbilityCost::SacrificeSelf]
            if cost.to_string() == "{7}"
    ));
    assert!(matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(filter),
        }] if filter.kind == TargetKind::AnyPermanent
            && filter.permanent_types.is_empty()
            && filter.controller == TargetController::Any
    ));
    let group = &ability.targeting.as_ref().unwrap().groups[0];
    assert_eq!((group.min, group.max), (1, 1));
}

#[test]
fn q65_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, face_id, fingerprint) in [
        (
            "fireslinger",
            "Fireslinger",
            "fireslinger",
            "f2c0e85d2d6313c69c95aee4aeae0a5a2ba09dd3e054947cd058b188438dfa08",
        ),
        (
            "goblin_fireslinger",
            "Goblin Fireslinger",
            "goblin_fireslinger",
            "10d641a84d8d881d032e8e807baed4bf77768d4d6d5a473be761a2007c1bbba0",
        ),
        (
            "valakut_invoker",
            "Valakut Invoker",
            "valakut_invoker",
            "0706d8f8bb8df43b78978756c12e3c320ca9a10f5e7222249fad9393466fd047",
        ),
        (
            "aeolipile",
            "Aeolipile",
            "aeolipile",
            "b68ac3b0d52115e53b6bd4b1c0fa5505775326655dc891a4ca91c7c738e01f2e",
        ),
        (
            "gavony_silversmith",
            "Gavony Silversmith",
            "gavony_silversmith",
            "22a3d4ffc39435cf6aad224c61faada5403c82da3b61184c18c4753af2370345",
        ),
        (
            "basris_acolyte",
            "Basri's Acolyte",
            "basris_acolyte",
            "c79c45083e66fde3abaf6206898738f414d19b3beeb7dfe9c63b5b2d32e09294",
        ),
        (
            "keen-eyed_raven",
            "Keen-Eyed Raven",
            "keen_eyed_raven",
            "bbf8fded966796f409e625e0ed359764b542e721218c71cbe3180a1e917876e0",
        ),
        (
            "supply-line_cranes",
            "Supply-Line Cranes",
            "supply_line_cranes",
            "110c795d76f45b9a77c3ae545e58d8f2cb480f49fa67e767972e24bdec316999",
        ),
        (
            "vindicate",
            "Vindicate",
            "vindicate",
            "94e9760605d1dbf0015ca518027935e0ad359e55ed0d7b81289e5ab9c566fc84",
        ),
        (
            "desert_twister",
            "Desert Twister",
            "desert_twister",
            "94e9760605d1dbf0015ca518027935e0ad359e55ed0d7b81289e5ab9c566fc84",
        ),
        (
            "universal_solvent",
            "Universal Solvent",
            "universal_solvent",
            "54874424d4869e6840848d0f38c10d4403d904eeb54b2ea19831192d6fa1c81a",
        ),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], face_id);
        assert_eq!(fields[3], name, "display name drift for {id}");
        assert_eq!(fields[4], fingerprint, "fingerprint drift for {id}");
    }
}
