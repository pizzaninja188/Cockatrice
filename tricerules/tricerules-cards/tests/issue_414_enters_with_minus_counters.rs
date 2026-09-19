//! Issue #414 registry conformance for the five retained enters-with-minus-counter identities.
//!
//! Each retained card is generated from exact typed recipes: CR 614.1c / 122.6 entry replacements
//! place one or two -1/-1 counters through `StaticAbilityDef::EntersWithCounters`, and the companion
//! sorcery-speed activations pay `{mana}` plus one or two counters from the exact source generation
//! as an atomic cost (CR 602.2b / 601.2h / 122.1) before applying the shipped typed effect.
//! "Remove two counters from this creature" (Gnarlbark Elm and Reaping Willow) is authored as two
//! any-one-counter components so mixed-kind payment stays exact. The remaining unmapped siblings
//! stay unretained.

use tricerules_cards::primitives::{
    CardTypeFilter, CounterRemovalPaymentSource, EffectSubject, EntersWithCountersAffected,
    GraveyardDestination, GraveyardFilter, GraveyardOwner, PlayerRecipient, StaticAbilityDef,
    TargetController, TargetFilter, TargetKind, TargetObjectExclusion, ZoneCardFilter,
};
use tricerules_cards::{
    AbilityCost, AbilitySourceZone, ActivationTiming, Amount, CardRegistry, CounterKind, Keyword,
    ManaCost, SpellEffectKind,
};

fn another_creature_you_control() -> TargetFilter {
    TargetFilter {
        kind: TargetKind::Creature,
        controller: TargetController::You,
        excluded_objects: vec![TargetObjectExclusion::Source],
        ..TargetFilter::default()
    }
}

fn remove_one_counter_from_source() -> AbilityCost {
    AbilityCost::RemoveCounters {
        counter: None,
        count: 1,
        payment_source: CounterRemovalPaymentSource::Source,
    }
}

/// "Remove two counters from this creature" is two atomic any-one-counter components so payment
/// may mix counter kinds (for example one -1/-1 plus one stun).
fn remove_two_counters_from_source() -> [AbilityCost; 2] {
    [
        remove_one_counter_from_source(),
        remove_one_counter_from_source(),
    ]
}

#[test]
fn issue_414_registers_the_five_completed_identities() {
    let registry = CardRegistry::global();
    for (id, name, mana_cost, types, stats, keywords) in [
        (
            "burdened_stoneback",
            "Burdened Stoneback",
            "{1}{W}",
            vec!["Creature", "Giant", "Warrior"],
            (4, 4),
            Vec::<Keyword>::new(),
        ),
        (
            "gnarlbark_elm",
            "Gnarlbark Elm",
            "{2}{B}",
            vec!["Creature", "Treefolk", "Warlock"],
            (3, 4),
            Vec::<Keyword>::new(),
        ),
        (
            "moonlit_lamenter",
            "Moonlit Lamenter",
            "{2}{W}",
            vec!["Creature", "Treefolk", "Cleric"],
            (2, 5),
            Vec::<Keyword>::new(),
        ),
        (
            "hovel_hurler",
            "Hovel Hurler",
            "{3}{R/W}{R/W}",
            vec!["Creature", "Giant", "Warrior"],
            (6, 7),
            Vec::<Keyword>::new(),
        ),
        (
            "reaping_willow",
            "Reaping Willow",
            "{1}{W/B}{W/B}{W/B}",
            vec!["Creature", "Treefolk", "Cleric"],
            (3, 6),
            vec![Keyword::Lifelink],
        ),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing retained card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id);
        assert_eq!(face.mana_cost.to_string(), mana_cost);
        assert_eq!(face.types, types);
        assert_eq!(face.power.zip(face.toughness), Some(stats));
        assert_eq!(face.keywords, keywords);
    }
}

#[test]
fn issue_414_excludes_the_unmapped_siblings() {
    let registry = CardRegistry::global();
    for name in [
        "Flitterwing Nuisance",
        "Glen Elendra Guardian",
        "Slumbering Walker",
    ] {
        assert_eq!(
            registry.id_for_name(name),
            None,
            "{name} must stay unretained until its missing capability lands"
        );
    }
}

#[test]
fn issue_414_entry_replacement_payloads_are_exact() {
    let registry = CardRegistry::global();
    for (id, expected_amount) in [
        ("burdened_stoneback", 2),
        ("gnarlbark_elm", 2),
        ("moonlit_lamenter", 1),
        ("hovel_hurler", 2),
        ("reaping_willow", 2),
    ] {
        let face = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing retained card {id}"))
            .primary_face();
        let [ability] = face.static_abilities.as_slice() else {
            panic!("{id} must have exactly one static ability");
        };
        assert_eq!(
            ability.definition,
            StaticAbilityDef::EntersWithCounters {
                affected: EntersWithCountersAffected::Self_,
                counter: CounterKind::MinusOneMinusOne,
                amount: Amount::Fixed(expected_amount),
                cast_cost_condition: None,
            },
            "{id}"
        );
    }
}

#[test]
fn issue_414_burdened_stoneback_activation_payload_is_exact() {
    let registry = CardRegistry::global();
    let face = registry
        .get("burdened_stoneback")
        .expect("Burdened Stoneback")
        .primary_face();
    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Burdened Stoneback must have exactly one activated ability");
    };
    assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(
        ability.costs,
        [
            AbilityCost::Mana(ManaCost::parse("{1}{W}").unwrap()),
            remove_one_counter_from_source(),
        ]
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::GrantKeywords {
            subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            keywords: vec![Keyword::Indestructible],
        }]
    );
    assert_eq!(ability.timing, ActivationTiming::SorcerySpeed);
    let targeting = ability.targeting.as_ref().expect("one target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("Burdened Stoneback must have exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target creature");
    assert_eq!(group.effect_indices, [0]);
}

#[test]
fn issue_414_moonlit_lamenter_activation_payload_is_exact() {
    let registry = CardRegistry::global();
    let face = registry
        .get("moonlit_lamenter")
        .expect("Moonlit Lamenter")
        .primary_face();
    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Moonlit Lamenter must have exactly one activated ability");
    };
    assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(
        ability.costs,
        [
            AbilityCost::Mana(ManaCost::parse("{1}{W}").unwrap()),
            remove_one_counter_from_source(),
        ]
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        }]
    );
    assert_eq!(ability.timing, ActivationTiming::SorcerySpeed);
    assert!(ability.targeting.is_none());
}

#[test]
fn issue_414_hovel_hurler_activation_payload_is_exact() {
    let registry = CardRegistry::global();
    let face = registry
        .get("hovel_hurler")
        .expect("Hovel Hurler")
        .primary_face();
    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Hovel Hurler must have exactly one activated ability");
    };
    assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(
        ability.costs,
        [
            AbilityCost::Mana(ManaCost::parse("{R/W}{R/W}").unwrap()),
            remove_one_counter_from_source(),
        ]
    );
    assert_eq!(
        ability.effect,
        [
            SpellEffectKind::PumpTarget {
                power: 1,
                toughness: 0,
                scale: None,
                subject: EffectSubject::Chosen(Box::new(another_creature_you_control())),
            },
            SpellEffectKind::GrantKeywords {
                subject: EffectSubject::Chosen(Box::new(another_creature_you_control())),
                keywords: vec![Keyword::Flying],
            },
        ]
    );
    assert_eq!(ability.timing, ActivationTiming::SorcerySpeed);
    let targeting = ability.targeting.as_ref().expect("one target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("Hovel Hurler must have exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose another target creature you control");
    assert_eq!(group.effect_indices, [0, 1]);
    assert!(group.distinct_from.is_empty());
}

#[test]
fn issue_414_gnarlbark_elm_activation_payload_is_exact() {
    let registry = CardRegistry::global();
    let face = registry
        .get("gnarlbark_elm")
        .expect("Gnarlbark Elm")
        .primary_face();
    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Gnarlbark Elm must have exactly one activated ability");
    };
    assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
    let mut expected_costs = vec![AbilityCost::Mana(ManaCost::parse("{2}{B}").unwrap())];
    expected_costs.extend(remove_two_counters_from_source());
    assert_eq!(ability.costs, expected_costs);
    assert_eq!(
        ability.effect,
        [SpellEffectKind::PumpTarget {
            power: -2,
            toughness: -2,
            scale: None,
            subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
        }]
    );
    assert_eq!(ability.timing, ActivationTiming::SorcerySpeed);
    let targeting = ability.targeting.as_ref().expect("one target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("Gnarlbark Elm must have exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target creature");
    assert_eq!(group.effect_indices, [0]);
}

#[test]
fn issue_414_reaping_willow_activation_payload_is_exact() {
    let registry = CardRegistry::global();
    let face = registry
        .get("reaping_willow")
        .expect("Reaping Willow")
        .primary_face();
    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Reaping Willow must have exactly one activated ability");
    };
    assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
    let mut expected_costs = vec![AbilityCost::Mana(ManaCost::parse("{1}{W/B}").unwrap())];
    expected_costs.extend(remove_two_counters_from_source());
    assert_eq!(ability.costs, expected_costs);
    assert_eq!(
        ability.effect,
        [SpellEffectKind::MoveGraveyardCards {
            filter: GraveyardFilter {
                owner: GraveyardOwner::Controller,
                card: Some(ZoneCardFilter {
                    card_type: Some(CardTypeFilter::Creature),
                    max_mana_value: Some(3),
                    ..ZoneCardFilter::default()
                }),
                ..GraveyardFilter::default()
            },
            destination: GraveyardDestination::Battlefield { tapped: false },
            linked_exile_id: None,
        }]
    );
    assert_eq!(ability.timing, ActivationTiming::SorcerySpeed);
    let targeting = ability.targeting.as_ref().expect("one target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("Reaping Willow must have exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(
        group.prompt,
        "Choose target creature card with mana value 3 or less from your graveyard"
    );
    assert_eq!(group.effect_indices, [0]);
}

#[test]
fn issue_414_fingerprint_rows_match_the_presentation_registry() {
    let registry = CardRegistry::global();
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, face_id) in [
        ("burdened_stoneback", "burdened_stoneback"),
        ("gnarlbark_elm", "gnarlbark_elm"),
        ("moonlit_lamenter", "moonlit_lamenter"),
        ("hovel_hurler", "hovel_hurler"),
        ("reaping_willow", "reaping_willow"),
    ] {
        let presentation = registry
            .presentation_face(id, face_id)
            .unwrap_or_else(|| panic!("missing presentation metadata for {id}/{face_id}"));
        assert_eq!(presentation.oracle_text_sha256.len(), 64);
        let row = fingerprints
            .lines()
            .find(|line| {
                line.starts_with(&format!("{id}\t")) && line.contains(&format!("\t{face_id}\t"))
            })
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}/{face_id}"));
        assert!(
            row.ends_with(&presentation.oracle_text_sha256),
            "fingerprint drift for {id}/{face_id}: {row}"
        );
    }
}

#[test]
fn issue_414_retained_identities_stay_creature_sources() {
    // Guard the exact printed type line: a mis-typed face would change creature-context recipes.
    let registry = CardRegistry::global();
    for id in [
        "burdened_stoneback",
        "gnarlbark_elm",
        "moonlit_lamenter",
        "hovel_hurler",
        "reaping_willow",
    ] {
        let face = registry.get(id).expect("retained identity").primary_face();
        assert!(
            face.types.iter().any(|card_type| card_type == "Creature"),
            "{id} must stay a creature"
        );
        assert!(
            !face.types.iter().any(|card_type| card_type == "Sorcery"),
            "{id} must not become a sorcery"
        );
    }
}
