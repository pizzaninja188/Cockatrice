//! Issue #415 registry conformance for the five retained Clue Equipment identities.
//!
//! Each card is generated from exact typed recipes: CR 602.2b / 601.2h / 701.21a make
//! `{2}, Sacrifice this Equipment: Draw a card.` one normal-timing battlefield activation whose
//! costs are an ordered mana payment and an atomic self-sacrifice; CR 301.5 / 702.6a supply the
//! `Equip` ability; and the shipped `AttachedModifier` surface carries each printed equipping
//! static. The granted attack trigger (Candlestick) and granted tap ability (Wrench) are nested
//! child nodes of their owning static with intentional presentation fallback.

use tricerules_cards::primitives::{
    CombatRestriction, GameCondition, LifeAmount, PlayerRecipient, RelativePlayerSet,
    SpellEffectKind, StaticAbilityDef, TargetController, TargetFilter, TargetKind,
    TypeLineAddition,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, AbilitySourceZone, ActivationTiming, Amount, CardRegistry,
    Keyword, LibraryPartitionKind, ManaCost, TriggerCondition,
};

const CARDS: &[(&str, &str, &str, &str)] = &[
    ("candlestick", "Candlestick", "{U}", "{2}"),
    ("knife", "Knife", "{R}", "{2}"),
    ("lead_pipe", "Lead Pipe", "{B}", "{2}"),
    ("rope", "Rope", "{G}", "{3}"),
    ("wrench", "Wrench", "{W}", "{2}"),
];

fn sacrifice_draw_ability_index(face: &tricerules_cards::CardFace) -> usize {
    face.activated_abilities
        .iter()
        .position(|ability| {
            ability
                .costs
                .iter()
                .any(|cost| matches!(cost, AbilityCost::SacrificeSelf))
        })
        .expect("every Clue Equipment prints the shared sacrifice-draw activation")
}

fn equip_ability_index(face: &tricerules_cards::CardFace) -> usize {
    face.activated_abilities
        .iter()
        .position(|ability| {
            ability
                .effect
                .iter()
                .any(|effect| matches!(effect, SpellEffectKind::Equip { .. }))
        })
        .expect("every Equipment prints an Equip ability")
}

#[test]
fn issue_415_registers_the_five_completed_identities() {
    let registry = CardRegistry::global();
    for (id, name, mana_cost, _) in CARDS {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing retained card {id}"));
        assert_eq!(&definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(*id));
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), *id);
        assert_eq!(face.mana_cost.to_string(), *mana_cost);
        assert_eq!(face.types, ["Artifact", "Clue", "Equipment"]);
        assert_eq!(face.power.zip(face.toughness), None);
    }
}

#[test]
fn issue_415_equip_abilities_match_the_printed_costs() {
    let registry = CardRegistry::global();
    for (id, name, _, equip_cost) in CARDS {
        let face = registry.get(id).expect("retained identity").primary_face();
        let index = equip_ability_index(face);
        let ability = &face.activated_abilities[index];
        assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
        assert_eq!(ability.timing, ActivationTiming::Normal);
        assert_eq!(
            ability.costs,
            [AbilityCost::Mana(
                ManaCost::parse(equip_cost).expect("printed equip cost")
            )],
            "{name} equip cost"
        );
        assert_eq!(
            ability.effect,
            [SpellEffectKind::Equip {
                target: TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::You,
                    ..TargetFilter::default()
                },
            }],
            "{name} equip effect"
        );
    }
}

#[test]
fn issue_415_shared_sacrifice_draw_activation_is_exact_and_ordered_before_equip() {
    let registry = CardRegistry::global();
    for (id, name, _, _) in CARDS {
        let face = registry.get(id).expect("retained identity").primary_face();
        let index = sacrifice_draw_ability_index(face);
        assert_eq!(index, 0, "{name} prints the sacrifice ability before Equip");
        let ability = &face.activated_abilities[index];
        assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
        assert_eq!(ability.timing, ActivationTiming::Normal);
        assert_eq!(
            ability.costs,
            [
                AbilityCost::Mana(ManaCost::parse("{2}").expect("printed mana cost")),
                AbilityCost::SacrificeSelf,
            ],
            "{name} pays mana then sacrifices the Equipment"
        );
        assert_eq!(
            ability.effect,
            [SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }],
            "{name} draws one card on resolution"
        );
        assert!(ability.targeting.is_none());
        assert!(ability.conditions.is_empty());
        assert!(ability.cost_modifiers.is_empty());
        assert!(ability.activation_limit.is_none());
    }
}

#[test]
fn issue_415_candlestick_static_grants_the_attack_surveil_trigger() {
    let registry = CardRegistry::global();
    let face = registry
        .get("candlestick")
        .expect("Candlestick")
        .primary_face();
    let [static_ability] = face.static_abilities.as_slice() else {
        panic!("Candlestick must have exactly one static ability");
    };
    assert_eq!(
        static_ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    let StaticAbilityDef::AttachedModifier {
        condition,
        add_types,
        set_types,
        set_name,
        set_colors,
        delta_power,
        delta_toughness,
        count,
        power_per_match,
        toughness_per_match,
        set_power,
        set_toughness,
        remove_all_abilities,
        keywords,
        triggered_abilities,
        activated_abilities,
        restriction,
        doesnt_untap_during_untap_step,
        cant_untap,
    } = &static_ability.definition
    else {
        panic!("Candlestick must attach a modifier to the equipped creature");
    };
    assert!(condition.is_none());
    assert_eq!(add_types, &TypeLineAddition::default());
    assert!(set_types.is_none());
    assert!(set_name.is_none());
    assert!(set_colors.is_none());
    assert_eq!((*delta_power, *delta_toughness), (1, 1));
    assert!(count.is_none());
    assert_eq!((*power_per_match, *toughness_per_match), (0, 0));
    assert!(set_power.is_none() && set_toughness.is_none());
    assert!(!remove_all_abilities);
    assert!(keywords.is_empty());
    assert!(activated_abilities.is_empty());
    assert_eq!(restriction, &CombatRestriction::default());
    assert!(!doesnt_untap_during_untap_step);
    assert!(!cant_untap);
    let [granted] = triggered_abilities.as_slice() else {
        panic!("Candlestick must grant exactly one triggered ability");
    };
    assert_eq!(
        granted.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0
        }
    );
    assert_eq!(granted.presentation, AbilityPresentation::Fallback);
    assert_eq!(
        granted.effect,
        [SpellEffectKind::LibraryPartition {
            count: 2,
            top_min: 0,
            top_max: None,
            kind: LibraryPartitionKind::Surveil,
        }]
    );
    assert!(!granted.may);
    assert!(granted.intervening_if.is_none());
    assert!(granted.targeting.is_none());
}

#[test]
fn issue_415_knife_static_is_conditioned_on_the_controllers_turn() {
    let registry = CardRegistry::global();
    let face = registry.get("knife").expect("Knife").primary_face();
    let [static_ability] = face.static_abilities.as_slice() else {
        panic!("Knife must have exactly one static ability");
    };
    assert_eq!(
        static_ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    let StaticAbilityDef::AttachedModifier {
        condition,
        delta_power,
        delta_toughness,
        keywords,
        triggered_abilities,
        activated_abilities,
        restriction,
        ..
    } = &static_ability.definition
    else {
        panic!("Knife must attach a modifier to the equipped creature");
    };
    assert_eq!(
        condition,
        &Some(GameCondition::ActivePlayer {
            players: RelativePlayerSet::Controller,
        })
    );
    assert_eq!((*delta_power, *delta_toughness), (1, 0));
    assert_eq!(keywords, &[Keyword::FirstStrike]);
    assert!(triggered_abilities.is_empty());
    assert!(activated_abilities.is_empty());
    assert!(restriction.is_empty());
}

#[test]
fn issue_415_lead_pipe_static_and_death_trigger_are_exact() {
    let registry = CardRegistry::global();
    let face = registry.get("lead_pipe").expect("Lead Pipe").primary_face();
    let [static_ability] = face.static_abilities.as_slice() else {
        panic!("Lead Pipe must have exactly one static ability");
    };
    assert_eq!(
        static_ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        static_ability.definition,
        StaticAbilityDef::AttachedModifier {
            condition: None,
            add_types: TypeLineAddition::default(),
            set_types: None,
            set_name: None,
            set_colors: None,
            delta_power: 2,
            delta_toughness: 0,
            count: None,
            power_per_match: 0,
            toughness_per_match: 0,
            set_power: None,
            set_toughness: None,
            remove_all_abilities: false,
            keywords: Vec::new(),
            triggered_abilities: Vec::new(),
            activated_abilities: Vec::new(),
            restriction: CombatRestriction::default(),
            doesnt_untap_during_untap_step: false,
            cant_untap: false,
        }
    );

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Lead Pipe must have exactly one triggered ability");
    };
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        trigger.trigger,
        TriggerCondition::WheneverAttachedObjectDies
    );
    assert_eq!(
        trigger.effect,
        [SpellEffectKind::LoseLife {
            amount: LifeAmount::Fixed(1),
            who: PlayerRecipient::EachOpponent,
        }]
    );
    assert!(!trigger.may);
    assert!(trigger.targeting.is_none());
    assert!(trigger.intervening_if.is_none());
}

#[test]
fn issue_415_rope_static_carries_reach_and_the_one_blocker_cap() {
    let registry = CardRegistry::global();
    let face = registry.get("rope").expect("Rope").primary_face();
    let [static_ability] = face.static_abilities.as_slice() else {
        panic!("Rope must have exactly one static ability");
    };
    assert_eq!(
        static_ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    let StaticAbilityDef::AttachedModifier {
        delta_power,
        delta_toughness,
        keywords,
        restriction,
        triggered_abilities,
        activated_abilities,
        ..
    } = &static_ability.definition
    else {
        panic!("Rope must attach a modifier to the equipped creature");
    };
    assert_eq!((*delta_power, *delta_toughness), (1, 2));
    assert_eq!(keywords, &[Keyword::Reach]);
    assert_eq!(
        restriction,
        &CombatRestriction {
            maximum_blockers: Some(1),
            ..CombatRestriction::default()
        }
    );
    assert!(triggered_abilities.is_empty());
    assert!(activated_abilities.is_empty());
}

#[test]
fn issue_415_wrench_static_grants_vigilance_and_the_tap_ability() {
    let registry = CardRegistry::global();
    let face = registry.get("wrench").expect("Wrench").primary_face();
    let [static_ability] = face.static_abilities.as_slice() else {
        panic!("Wrench must have exactly one static ability");
    };
    assert_eq!(
        static_ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    let StaticAbilityDef::AttachedModifier {
        delta_power,
        delta_toughness,
        keywords,
        activated_abilities,
        triggered_abilities,
        restriction,
        ..
    } = &static_ability.definition
    else {
        panic!("Wrench must attach a modifier to the equipped creature");
    };
    assert_eq!((*delta_power, *delta_toughness), (1, 1));
    assert_eq!(keywords, &[Keyword::Vigilance]);
    assert!(triggered_abilities.is_empty());
    assert!(restriction.is_empty());
    let [granted] = activated_abilities.as_slice() else {
        panic!("Wrench must grant exactly one activated ability");
    };
    assert_eq!(granted.presentation, AbilityPresentation::Fallback);
    assert_eq!(granted.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(granted.timing, ActivationTiming::Normal);
    assert_eq!(
        granted.costs,
        [
            AbilityCost::Mana(ManaCost::parse("{3}").expect("granted mana cost")),
            AbilityCost::Tap,
        ]
    );
    assert_eq!(
        granted.effect,
        [SpellEffectKind::Tap {
            subject: tricerules_cards::primitives::EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                ..TargetFilter::default()
            })),
        }]
    );
    let targeting = granted
        .targeting
        .as_ref()
        .expect("granted tap target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("Wrench grants exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target creature");
    assert_eq!(group.effect_indices, [0]);
}

#[test]
fn issue_415_fingerprint_rows_match_the_presentation_registry() {
    let registry = CardRegistry::global();
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, _, _, _) in CARDS {
        let presentation = registry
            .presentation_face(id, id)
            .unwrap_or_else(|| panic!("missing presentation metadata for {id}"));
        assert_eq!(presentation.oracle_text_sha256.len(), 64);
        let row = fingerprints
            .lines()
            .find(|line| {
                line.starts_with(&format!("{id}\t")) && line.contains(&format!("\t{id}\t"))
            })
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        assert!(
            row.ends_with(&presentation.oracle_text_sha256),
            "fingerprint drift for {id}: {row}"
        );
    }
}
