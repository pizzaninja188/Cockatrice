//! Registry and presentation conformance for issue #323.
//!
//! The eight exact generator templates must register the eight reviewed Standard identities with
//! their complete typed payloads: CR 701.7 mass destruction, CR 701.6/118.12a the {2} unless-pays
//! counter, CR 702.4 double strike, CR 111.10a the predefined Treasure, CR 701.23/614.1d the
//! basic-land search onto the battlefield tapped, CR 611.3 the trample anthem excluding the
//! source, CR 509.1b the unblockable grant, and CR 509.1b the single-blocker maximum.

use tricerules_cards::primitives::{
    CombatRestriction, CombatRestrictionScope, CreatureScopeController, CreatureScopeFilter,
    EffectSubject, PlayerRecipient, SearchDestination, SearchZoneSelection, SpellEffectKind,
    StackSpellFilter, StaticAbilityDef, TargetFilter, TargetGroupDef, TargetSchema, ZoneCardFilter,
};
use tricerules_cards::{
    AbilityPresentation, Amount, CardRegistry, Color, Keyword, Layout, TriggerCondition,
};

fn single_group(targeting: &tricerules_cards::primitives::TargetingDef) -> &TargetGroupDef {
    let [group] = targeting.groups.as_slice() else {
        panic!("expected exactly one authored target group");
    };
    group
}

fn treasure_effect() -> SpellEffectKind {
    SpellEffectKind::CreateTokens {
        token: "treasure".into(),
        count: Amount::Fixed(1),
        who: PlayerRecipient::Controller,
        tapped: false,
        sacrifice_timing: None,
    }
}

fn basic_land_search_effect() -> SpellEffectKind {
    SpellEffectKind::SearchLibrary {
        who: PlayerRecipient::Controller,
        optional: false,
        count: 1,
        count_by_cast_cost: None,
        filter: Some(ZoneCardFilter {
            card_type: Some(tricerules_cards::primitives::CardTypeFilter::BasicLand),
            ..ZoneCardFilter::default()
        }),
        slots: Vec::new(),
        zones: SearchZoneSelection::default(),
        destination: SearchDestination::Battlefield { tapped: true },
        conditional_destination: None,
        shuffle: true,
        reveal: false,
        result_id: None,
    }
}

#[test]
fn issue_323_registers_the_eight_reviewed_identities() {
    let registry = CardRegistry::global();
    for (id, name, face_count, layout) in [
        ("day_of_judgment", "Day of Judgment", 1, Layout::Normal),
        ("itll_quench_ya!", "It'll Quench Ya!", 1, Layout::Normal),
        (
            "two-headed_hunter_twice_the_rage",
            "Two-Headed Hunter // Twice the Rage",
            2,
            Layout::Adventure,
        ),
        ("ancestors_aid", "Ancestors' Aid", 1, Layout::Normal),
        ("shared_roots", "Shared Roots", 1, Layout::Normal),
        (
            "aggressive_mammoth",
            "Aggressive Mammoth",
            1,
            Layout::Normal,
        ),
        ("enter_the_enigma", "Enter the Enigma", 1, Layout::Normal),
        (
            "professional_wrestler",
            "Professional Wrestler",
            1,
            Layout::Normal,
        ),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing reviewed card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        assert_eq!(definition.layout, layout);
        assert_eq!(definition.face_count(), face_count);
    }
}

#[test]
fn issue_323_day_of_judgment_is_the_untargeted_mass_destroy() {
    let definition = CardRegistry::global()
        .get("day_of_judgment")
        .expect("Day of Judgment");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "day_of_judgment");
    assert_eq!(face.mana_cost.to_string(), "{2}{W}{W}");
    assert_eq!(face.types, ["Sorcery"]);
    assert_eq!(face.colors(), vec![Color::White]);
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::DestroyAll {
            kind: TargetFilter::default_creature(),
            prevent_regeneration: false,
        }]
    );
    assert!(face.targeting.is_none());
    assert!(TargetSchema::compile(&face.spell_effect, None).is_ok());
}

#[test]
fn issue_323_itll_quench_ya_counters_unless_two_is_paid() {
    let definition = CardRegistry::global()
        .get("itll_quench_ya!")
        .expect("It'll Quench Ya!");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "it_ll_quench_ya");
    assert_eq!(face.mana_cost.to_string(), "{1}{U}");
    assert_eq!(face.types, ["Instant", "Lesson"]);
    assert_eq!(face.colors(), vec![Color::Blue]);
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::CounterTargetSpell {
            spell_filter: StackSpellFilter::default(),
            unless_controller_pays: Some(Amount::Fixed(2)),
            unless_controller_pays_by_cast_cost: None,
        }]
    );
    assert!(face.targeting.is_none());
    assert!(TargetSchema::compile(&face.spell_effect, None).is_ok());
}

#[test]
fn issue_323_twice_the_rage_grants_double_strike() {
    let definition = CardRegistry::global()
        .get("two-headed_hunter_twice_the_rage")
        .expect("Two-Headed Hunter // Twice the Rage");
    assert_eq!(definition.layout, Layout::Adventure);

    let creature = definition.primary_face();
    assert_eq!(creature.face_id.as_str(), "two_headed_hunter");
    assert_eq!(creature.mana_cost.to_string(), "{4}{R}");
    assert_eq!(creature.types, ["Creature", "Giant"]);
    assert_eq!((creature.power, creature.toughness), (Some(5), Some(4)));
    assert_eq!(creature.colors(), vec![Color::Red]);
    assert_eq!(creature.keywords, [Keyword::Menace]);
    assert!(creature.spell_effect.is_empty());
    assert!(creature.triggered_abilities.is_empty());

    let adventure = definition.face(1).expect("Twice the Rage face");
    assert_eq!(adventure.face_id.as_str(), "twice_the_rage");
    assert_eq!(adventure.mana_cost.to_string(), "{1}{R}");
    assert_eq!(adventure.types, ["Instant", "Adventure"]);
    assert_eq!(adventure.colors(), vec![Color::Red]);
    assert_eq!(
        adventure.spell_effect,
        [SpellEffectKind::GrantKeywords {
            subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            keywords: vec![Keyword::DoubleStrike],
        }]
    );
    let targeting = adventure
        .targeting
        .as_ref()
        .expect("Twice the Rage targets a creature");
    let group = single_group(targeting);
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target creature");
    assert_eq!(group.effect_indices, [0]);
    assert!(group.distinct_from.is_empty());
    assert!(TargetSchema::compile(&adventure.spell_effect, Some(targeting)).is_ok());
}

#[test]
fn issue_323_ancestors_aid_pumps_and_creates_a_treasure() {
    let definition = CardRegistry::global()
        .get("ancestors_aid")
        .expect("Ancestors' Aid");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "ancestors_aid");
    assert_eq!(face.mana_cost.to_string(), "{1}{R}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::Red]);
    assert_eq!(
        face.spell_effect,
        [
            SpellEffectKind::PumpTarget {
                power: 2,
                toughness: 0,
                scale: None,
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            },
            SpellEffectKind::GrantKeywords {
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                keywords: vec![Keyword::FirstStrike],
            },
            treasure_effect(),
        ]
    );
    let targeting = face
        .targeting
        .as_ref()
        .expect("Ancestors' Aid targets a creature");
    let group = single_group(targeting);
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target creature");
    assert_eq!(group.effect_indices, [0, 1]);
    assert!(TargetSchema::compile(&face.spell_effect, Some(targeting)).is_ok());
}

#[test]
fn issue_323_shared_roots_searches_a_tapped_basic_land() {
    let definition = CardRegistry::global()
        .get("shared_roots")
        .expect("Shared Roots");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "shared_roots");
    assert_eq!(face.mana_cost.to_string(), "{1}{G}");
    assert_eq!(face.types, ["Sorcery", "Lesson"]);
    assert_eq!(face.colors(), vec![Color::Green]);
    assert_eq!(face.spell_effect, [basic_land_search_effect()]);
    assert!(face.targeting.is_none());
}

#[test]
fn issue_323_aggressive_mammoth_anthems_trample_to_other_creatures() {
    let definition = CardRegistry::global()
        .get("aggressive_mammoth")
        .expect("Aggressive Mammoth");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "aggressive_mammoth");
    assert_eq!(face.mana_cost.to_string(), "{3}{G}{G}{G}");
    assert_eq!(face.types, ["Creature", "Elephant"]);
    assert_eq!((face.power, face.toughness), (Some(8), Some(8)));
    assert_eq!(face.colors(), vec![Color::Green]);
    assert_eq!(face.keywords, [Keyword::Trample]);

    let [ability] = face.static_abilities.as_slice() else {
        panic!("Aggressive Mammoth must have exactly one static ability");
    };
    assert_eq!(ability.ability_id.as_str(), "static_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.definition,
        StaticAbilityDef::AnthemKeyword {
            filter: CreatureScopeFilter {
                controller: Some(CreatureScopeController::YouControl),
                exclude_self: true,
                ..CreatureScopeFilter::default()
            },
            condition: None,
            keyword: Keyword::Trample,
        }
    );
}

#[test]
fn issue_323_enter_the_enigma_makes_a_creature_unblockable_and_draws() {
    let definition = CardRegistry::global()
        .get("enter_the_enigma")
        .expect("Enter the Enigma");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "enter_the_enigma");
    assert_eq!(face.mana_cost.to_string(), "{U}");
    assert_eq!(face.types, ["Sorcery"]);
    assert_eq!(face.colors(), vec![Color::Blue]);
    assert_eq!(
        face.spell_effect,
        [
            SpellEffectKind::ApplyCombatRestriction {
                scope: CombatRestrictionScope::Chosen(TargetFilter::default_creature()),
                restriction: CombatRestriction {
                    cant_be_blocked: true,
                    ..CombatRestriction::default()
                },
            },
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            },
        ]
    );
    let targeting = face
        .targeting
        .as_ref()
        .expect("Enter the Enigma targets a creature");
    let group = single_group(targeting);
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target creature");
    assert_eq!(group.effect_indices, [0]);
    assert!(TargetSchema::compile(&face.spell_effect, Some(targeting)).is_ok());
}

#[test]
fn issue_323_professional_wrestler_keeps_treasure_and_max_one_blocker() {
    let definition = CardRegistry::global()
        .get("professional_wrestler")
        .expect("Professional Wrestler");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "professional_wrestler");
    assert_eq!(face.mana_cost.to_string(), "{3}{G}");
    assert_eq!(face.types, ["Creature", "Human", "Warrior", "Performer"]);
    assert_eq!((face.power, face.toughness), (Some(4), Some(4)));
    assert_eq!(face.colors(), vec![Color::Green]);
    assert!(face.spell_effect.is_empty());

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Professional Wrestler must have exactly one ETB ability");
    };
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(!ability.may);
    assert!(ability.intervening_if.is_none());
    assert!(ability.targeting.is_none());
    assert_eq!(ability.effect, [treasure_effect()]);

    let [restriction] = face.static_abilities.as_slice() else {
        panic!("Professional Wrestler must have exactly one static ability");
    };
    assert_eq!(restriction.ability_id.as_str(), "static_01");
    assert_eq!(
        restriction.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        restriction.definition,
        StaticAbilityDef::SelfCombatRestriction {
            restriction: CombatRestriction {
                maximum_blockers: Some(1),
                ..CombatRestriction::default()
            },
            condition: None,
        }
    );
}

#[test]
fn issue_323_fingerprint_rows_match_the_presentation_registry() {
    let registry = CardRegistry::global();
    let cases: &[(&str, &str, &str, &str)] = &[
        (
            "day_of_judgment",
            "day_of_judgment",
            "Day of Judgment",
            "Day of Judgment",
        ),
        (
            "itll_quench_ya!",
            "it_ll_quench_ya",
            "It'll Quench Ya!",
            "It'll Quench Ya!",
        ),
        (
            "two-headed_hunter_twice_the_rage",
            "two_headed_hunter",
            "Two-Headed Hunter // Twice the Rage",
            "Two-Headed Hunter",
        ),
        (
            "two-headed_hunter_twice_the_rage",
            "twice_the_rage",
            "Two-Headed Hunter // Twice the Rage",
            "Twice the Rage",
        ),
        (
            "ancestors_aid",
            "ancestors_aid",
            "Ancestors' Aid",
            "Ancestors' Aid",
        ),
        (
            "shared_roots",
            "shared_roots",
            "Shared Roots",
            "Shared Roots",
        ),
        (
            "aggressive_mammoth",
            "aggressive_mammoth",
            "Aggressive Mammoth",
            "Aggressive Mammoth",
        ),
        (
            "enter_the_enigma",
            "enter_the_enigma",
            "Enter the Enigma",
            "Enter the Enigma",
        ),
        (
            "professional_wrestler",
            "professional_wrestler",
            "Professional Wrestler",
            "Professional Wrestler",
        ),
    ];
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, face_id, card_name, face_name) in cases {
        let presentation = registry
            .presentation_face(id, face_id)
            .unwrap_or_else(|| panic!("missing presentation metadata for {id}/{face_id}"));
        assert_eq!(presentation.card_name, *card_name);
        assert_eq!(presentation.face_name, *face_name);
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
