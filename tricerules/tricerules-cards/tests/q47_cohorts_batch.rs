//! Registry conformance for the queue-v47 reuse-only cohorts.
#![allow(clippy::type_complexity)]
use tricerules_cards::mana::ManaCost;
use tricerules_cards::primitives::{
    AbilityCost, Amount, CardTypeFilter, CastTriggerPlayer, CounterKind, EffectSubject, Keyword,
    PermanentTypeFilter, PowerComparison, SpellEffectKind, StaticAbilityDef, TargetKind,
    TriggerCondition,
};
use tricerules_cards::{registry, AbilityPresentation, CardDefinition, CardFace, Layout};

fn definition(id: &str) -> &'static CardDefinition {
    registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
}

fn identity(
    id: &str,
    name: &str,
    mana: &str,
    types: &[&str],
    body: Option<(u32, u32)>,
) -> &'static CardFace {
    let definition = definition(id);
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(registry::global().id_for_name(name), Some(id), "{id}");
    let f = definition.primary_face();
    assert_eq!(f.face_id.as_str(), id, "{id} face id");
    assert_eq!(f.name, name, "{id} name");
    assert_eq!(f.mana_cost.to_string(), mana, "{id} mana");
    assert_eq!(
        f.types.iter().map(String::as_str).collect::<Vec<_>>(),
        types,
        "{id} types"
    );
    assert_eq!(
        (f.power, f.toughness),
        body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
        "{id} body"
    );
    f
}

// ── 01 landfall counter ─────────────────────────────────────────────────────

#[test]
fn q47_landfall_counter_maps_definitions() {
    for (id, name, mana, types, body, keyword, line) in [
        (
            "vinelasher_kudzu",
            "Vinelasher Kudzu",
            "{1}{G}",
            vec!["Creature", "Plant"],
            (1, 1),
            None,
            1,
        ),
        (
            "xindi_surveyors",
            "Xindi Surveyors",
            "{2}{G}",
            vec!["Creature", "Xindi", "Scientist"],
            (3, 2),
            None,
            1,
        ),
        (
            "prowling_felidar",
            "Prowling Felidar",
            "{3}{W}",
            vec!["Creature", "Cat", "Beast"],
            (2, 3),
            Some(Keyword::Vigilance),
            2,
        ),
        (
            "territorial_scythecat",
            "Territorial Scythecat",
            "{2}{G}",
            vec!["Creature", "Cat"],
            (2, 1),
            Some(Keyword::Trample),
            2,
        ),
    ] {
        let f = identity(id, name, mana, &types, Some(body));
        if let Some(keyword) = keyword {
            assert!(f.keywords.contains(&keyword), "{id}");
        }
        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{id}: one landfall trigger");
        };
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![line])
        );
        assert!(
            matches!(
                trigger.trigger,
                TriggerCondition::WheneverPermanentEntersBattlefield {
                    controller: CastTriggerPlayer::Controller,
                    filter: tricerules_cards::primitives::PermanentEventFilter {
                        permanent_type: Some(PermanentTypeFilter::Land),
                        ..
                    },
                    ..
                }
            ),
            "{id}: controller-scoped landfall"
        );
        assert!(matches!(
            trigger.effect.as_slice(),
            [SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                subject: EffectSubject::Source,
            }]
        ));
    }
}

// ── 02 noncreature spell counter ────────────────────────────────────────────

#[test]
fn q47_noncreature_counter_maps_definitions() {
    for (id, name, mana, types, body, keywords, line) in [
        (
            "deeproot_champion",
            "Deeproot Champion",
            "{1}{G}",
            vec!["Creature", "Merfolk", "Shaman"],
            (1, 1),
            vec![],
            1,
        ),
        (
            "pyroceratops",
            "Pyroceratops",
            "{3}{R}",
            vec!["Creature", "Elemental", "Dinosaur"],
            (2, 3),
            vec![Keyword::Trample],
            2,
        ),
        (
            "sprite_dragon",
            "Sprite Dragon",
            "{U}{R}",
            vec!["Creature", "Faerie", "Dragon"],
            (1, 1),
            vec![Keyword::Flying, Keyword::Haste],
            2,
        ),
    ] {
        let f = identity(id, name, mana, &types, Some(body));
        for keyword in keywords {
            assert!(f.keywords.contains(&keyword), "{id}");
        }
        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{id}: one cast trigger");
        };
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![line])
        );
        assert!(
            matches!(
                trigger.trigger,
                TriggerCondition::WheneverPlayerCastsSpell {
                    caster: CastTriggerPlayer::Controller,
                    filter: tricerules_cards::primitives::SpellCastFilter {
                        card_type: Some(CardTypeFilter::Noncreature),
                        ..
                    },
                    ordinal: None,
                    ..
                }
            ),
            "{id}: noncreature cast trigger"
        );
        assert!(matches!(
            trigger.effect.as_slice(),
            [SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                subject: EffectSubject::Source,
            }]
        ));
    }
}

// ── 03 second draw counter ──────────────────────────────────────────────────

#[test]
fn q47_second_draw_counter_maps_definitions() {
    for (id, name, mana, types, body, keywords, line) in [
        (
            "faerie_vandal",
            "Faerie Vandal",
            "{1}{U}",
            vec!["Creature", "Faerie", "Rogue"],
            (1, 2),
            vec![Keyword::Flying, Keyword::Flash],
            3,
        ),
        (
            "knights_of_dol_amroth",
            "Knights of Dol Amroth",
            "{3}{U}",
            vec!["Creature", "Human", "Knight"],
            (3, 3),
            vec![],
            1,
        ),
        (
            "thopter_mechanic",
            "Thopter Mechanic",
            "{1}{U}",
            vec!["Creature", "Human", "Artificer"],
            (2, 1),
            vec![],
            1,
        ),
    ] {
        let f = identity(id, name, mana, &types, Some(body));
        for keyword in keywords {
            assert!(f.keywords.contains(&keyword), "{id}");
        }
        let draw_trigger = f
            .triggered_abilities
            .iter()
            .find(|ability| {
                matches!(
                    ability.trigger,
                    TriggerCondition::WheneverPlayerDrawsNthCard {
                        drawer: CastTriggerPlayer::Controller,
                        ordinal: 2,
                    }
                )
            })
            .unwrap_or_else(|| panic!("{id}: second-draw trigger"));
        assert_eq!(
            draw_trigger.presentation,
            AbilityPresentation::OracleLines(vec![line])
        );
        assert!(matches!(
            draw_trigger.effect.as_slice(),
            [SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                subject: EffectSubject::Source,
            }]
        ));
        if id == "thopter_mechanic" {
            let dies = f
                .triggered_abilities
                .iter()
                .find(|ability| matches!(ability.trigger, TriggerCondition::WhenSelfDies))
                .expect("dies trigger");
            assert_eq!(dies.presentation, AbilityPresentation::OracleLines(vec![2]));
            assert!(matches!(
                dies.effect.as_slice(),
                [SpellEffectKind::CreateTokens { token, count: Amount::Fixed(1), .. }]
                    if token == "thopter_c_1_1_flying"
            ));
        }
    }
}

// ── 04 team trample ─────────────────────────────────────────────────────────

#[test]
fn q47_team_trample_maps_definitions() {
    for (id, name, mana, types, body) in [
        (
            "bloodcrusher_of_khorne",
            "Bloodcrusher of Khorne",
            "{3}{R}",
            vec!["Creature", "Demon", "Knight"],
            (3, 3),
        ),
        (
            "khenra_charioteer",
            "Khenra Charioteer",
            "{1}{R}{G}",
            vec!["Creature", "Jackal", "Warrior"],
            (3, 3),
        ),
        (
            "nyleas_forerunner",
            "Nylea's Forerunner",
            "{4}{G}",
            vec!["Enchantment", "Creature", "Beast"],
            (5, 3),
        ),
    ] {
        let f = identity(id, name, mana, &types, Some(body));
        assert!(f.keywords.contains(&Keyword::Trample), "{id}");
        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{id}: one anthem");
        };
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert!(
            matches!(
                &static_ability.definition,
                StaticAbilityDef::AnthemKeyword {
                    filter,
                    keyword: Keyword::Trample,
                    ..
                } if filter.exclude_self && filter.controller.is_some()
            ),
            "{id}: other controlled creatures gain trample"
        );
    }
}

// ── 05 basic landcycling spells ─────────────────────────────────────────────

#[test]
fn q47_basic_landcycling_spells_maps_definitions() {
    for (id, name, mana, kind, cycle_cost) in [
        ("absorb_vis", "Absorb Vis", "{6}{B}", "Sorcery", "{1}{B}"),
        ("fiery_fall", "Fiery Fall", "{5}{R}", "Instant", "{1}{R}"),
        (
            "mental_journey",
            "Mental Journey",
            "{4}{U}{U}",
            "Instant",
            "{1}{U}",
        ),
        (
            "sylvan_bounty",
            "Sylvan Bounty",
            "{5}{G}",
            "Instant",
            "{1}{G}",
        ),
    ] {
        let f = identity(id, name, mana, &[kind], None);
        match id {
            "absorb_vis" => assert!(matches!(
                f.spell_effect.as_slice(),
                [
                    SpellEffectKind::TargetPlayerLosesLife {
                        amount: 4,
                        target: tricerules_cards::primitives::ObjectFilter {
                            kind: TargetKind::AnyPlayer,
                            ..
                        },
                    },
                    SpellEffectKind::GainLife {
                        amount: Amount::Fixed(4),
                        ..
                    },
                ]
            )),
            "fiery_fall" => assert!(matches!(
                f.spell_effect.as_slice(),
                [SpellEffectKind::DamageTarget {
                    amount: Amount::Fixed(5),
                    target: tricerules_cards::primitives::ObjectFilter {
                        kind: TargetKind::Creature,
                        ..
                    },
                }]
            )),
            "mental_journey" => assert!(matches!(
                f.spell_effect.as_slice(),
                [SpellEffectKind::Draw {
                    count: Amount::Fixed(3),
                    ..
                }]
            )),
            "sylvan_bounty" => assert!(matches!(
                f.spell_effect.as_slice(),
                [SpellEffectKind::TargetPlayerGainsLife {
                    amount: Amount::Fixed(8),
                    target: tricerules_cards::primitives::ObjectFilter {
                        kind: TargetKind::AnyPlayer,
                        ..
                    },
                }]
            )),
            _ => unreachable!(),
        }
        let [cycling] = f.activated_abilities.as_slice() else {
            panic!("{id}: one basic-landcycling ability");
        };
        assert_eq!(
            cycling.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert_eq!(
            cycling.costs,
            vec![
                AbilityCost::Mana(ManaCost::parse(cycle_cost).expect("cycle mana")),
                AbilityCost::DiscardSelf,
            ]
        );
        assert!(matches!(
            cycling.effect.as_slice(),
            [SpellEffectKind::SearchLibrary { filter: Some(filter), .. }]
                if filter.card_type == Some(CardTypeFilter::BasicLand)
        ));
    }
}

// ── 06 pinger auras ─────────────────────────────────────────────────────────

#[test]
fn q47_pinger_aura_maps_definitions() {
    for (id, name, mana, restrict_own, sacrifice_line) in [
        ("power_of_fire", "Power of Fire", "{1}{R}", false, None),
        ("psionic_gift", "Psionic Gift", "{1}{U}", false, None),
        ("fire_whip", "Fire Whip", "{1}{R}", true, Some(3)),
    ] {
        let f = identity(id, name, mana, &["Enchantment", "Aura"], None);
        assert!(
            matches!(
                f.spell_effect.as_slice(),
                [SpellEffectKind::AuraAttach { target }]
                    if restrict_own
                        == matches!(
                            target,
                            tricerules_cards::primitives::ObjectFilter {
                                kind: TargetKind::Creature,
                                controller: tricerules_cards::primitives::TargetController::You,
                                ..
                            }
                        )
            ),
            "{id}: attachment target"
        );
        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{id}: one granted-ability static");
        };
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert!(matches!(
            static_ability.definition,
            StaticAbilityDef::AttachedModifier { .. }
        ));
        let StaticAbilityDef::AttachedModifier {
            activated_abilities,
            ..
        } = &static_ability.definition
        else {
            unreachable!();
        };
        assert!(
            activated_abilities.len() == 1
                && activated_abilities[0].costs == vec![AbilityCost::Tap]
                && matches!(
                    activated_abilities[0].effect.as_slice(),
                    [SpellEffectKind::DamageTarget {
                        amount: Amount::Fixed(1),
                        target: tricerules_cards::primitives::ObjectFilter {
                            kind: TargetKind::AnyTarget,
                            ..
                        },
                    }]
                ),
            "{id}: granted tap ping"
        );
        if let Some(line) = sacrifice_line {
            let [sacrifice] = f.activated_abilities.as_slice() else {
                panic!("{id}: one sacrifice ability");
            };
            assert_eq!(
                sacrifice.presentation,
                AbilityPresentation::OracleLines(vec![line])
            );
            assert_eq!(sacrifice.costs, vec![AbilityCost::SacrificeSelf]);
            assert!(matches!(
                sacrifice.effect.as_slice(),
                [SpellEffectKind::DamageTarget {
                    amount: Amount::Fixed(1),
                    ..
                }]
            ));
        }
    }
}

// ── 07 sweeper damage ───────────────────────────────────────────────────────

#[test]
fn q47_sweeper_damage_maps_definitions() {
    for (id, name, mana, kind, amount, excludes_flying, cycles) in [
        (
            "desert_sandstorm",
            "Desert Sandstorm",
            "{2}{R}",
            "Sorcery",
            1,
            false,
            false,
        ),
        (
            "sweltering_suns",
            "Sweltering Suns",
            "{1}{R}{R}",
            "Sorcery",
            3,
            false,
            true,
        ),
        (
            "seismic_shudder",
            "Seismic Shudder",
            "{1}{R}",
            "Instant",
            1,
            true,
            false,
        ),
        ("tremor", "Tremor", "{R}", "Sorcery", 1, true, false),
    ] {
        let f = identity(id, name, mana, &[kind], None);
        assert!(
            matches!(
                f.spell_effect.as_slice(),
                [SpellEffectKind::DamageAll { amount: a, kind: filter, .. }]
                    if *a == Amount::Fixed(amount)
                        && filter.excluded_keywords.contains(&Keyword::Flying) == excludes_flying
            ),
            "{id}: printed sweep"
        );
        if cycles {
            let [cycling] = f.activated_abilities.as_slice() else {
                panic!("{id}: one cycling ability");
            };
            assert_eq!(
                cycling.presentation,
                AbilityPresentation::OracleLines(vec![2])
            );
            assert_eq!(
                cycling.costs,
                vec![
                    AbilityCost::Mana(ManaCost::parse("{3}").expect("cycle mana")),
                    AbilityCost::DiscardSelf,
                ]
            );
            assert!(matches!(
                cycling.effect.as_slice(),
                [SpellEffectKind::Draw {
                    count: Amount::Fixed(1),
                    ..
                }]
            ));
        } else {
            assert!(f.activated_abilities.is_empty(), "{id}");
        }
    }
}

// ── 08 filtered destruction ─────────────────────────────────────────────────

#[test]
fn q47_filtered_destruction_maps_definitions() {
    for (id, name, mana, kind, filter_check) in [
        ("defeat", "Defeat", "{1}{B}", "Sorcery", "power_at_most_2"),
        (
            "smite_the_monstrous",
            "Smite the Monstrous",
            "{3}{W}",
            "Instant",
            "power_at_least_4",
        ),
        ("neck_snap", "Neck Snap", "{3}{W}", "Instant", "combat"),
        (
            "daring_demolition",
            "Daring Demolition",
            "{2}{B}{B}",
            "Sorcery",
            "creature_or_vehicle",
        ),
        (
            "dreadbore",
            "Dreadbore",
            "{B}{R}",
            "Sorcery",
            "creature_or_planeswalker",
        ),
    ] {
        let f = identity(id, name, mana, &[kind], None);
        let [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(filter),
        }] = f.spell_effect.as_slice()
        else {
            panic!("{id}: one chosen destroy");
        };
        match filter_check {
            "power_at_most_2" => assert!(
                matches!(filter.power, Some(PowerComparison::AtMost(2))),
                "{id}"
            ),
            "power_at_least_4" => assert!(
                matches!(filter.power, Some(PowerComparison::AtLeast(4))),
                "{id}"
            ),
            "combat" => assert_eq!(
                filter.combat_role,
                Some(tricerules_cards::primitives::CombatRole::AttackingOrBlocking),
                "{id}"
            ),
            "creature_or_vehicle" => assert!(
                filter.any_of.as_ref().is_some_and(|leaves| {
                    leaves.iter().any(|leaf| leaf.kind == TargetKind::Creature)
                        && leaves.iter().any(|leaf| {
                            leaf.required_subtypes == ["Vehicle"]
                                && leaf.kind == TargetKind::AnyPermanent
                        })
                }),
                "{id}"
            ),
            "creature_or_planeswalker" => assert_eq!(
                filter.permanent_types,
                vec![
                    PermanentTypeFilter::Creature,
                    PermanentTypeFilter::Planeswalker
                ],
                "{id}"
            ),
            _ => unreachable!(),
        }
    }
}

// ── 09 bite spells ──────────────────────────────────────────────────────────

#[test]
fn q47_bite_spells_maps_definitions() {
    for (id, name, mana, kind, target_kind, target_types, distinct) in [
        (
            "aggressive_instinct",
            "Aggressive Instinct",
            "{1}{G}",
            "Sorcery",
            TargetKind::Creature,
            vec![],
            true,
        ),
        (
            "masters_rebuke",
            "Master's Rebuke",
            "{1}{G}",
            "Instant",
            TargetKind::AnyPermanent,
            vec![
                PermanentTypeFilter::Creature,
                PermanentTypeFilter::Planeswalker,
            ],
            true,
        ),
        (
            "tail_slash",
            "Tail Slash",
            "{2}{R}",
            "Instant",
            TargetKind::Creature,
            vec![],
            true,
        ),
    ] {
        let f = identity(id, name, mana, &[kind], None);
        let [SpellEffectKind::CreatureDealsDamageEqualToPower { source, target }] =
            f.spell_effect.as_slice()
        else {
            panic!("{id}: one power-based damage instruction");
        };
        assert!(
            matches!(
                source,
                tricerules_cards::primitives::ObjectFilter {
                    kind: TargetKind::Creature,
                    controller: tricerules_cards::primitives::TargetController::You,
                    ..
                }
            ),
            "{id}: source is your creature"
        );
        match id {
            "masters_rebuke" => assert!(
                matches!(
                    target,
                    tricerules_cards::primitives::ObjectFilter {
                        kind: TargetKind::AnyPermanent,
                        controller: tricerules_cards::primitives::TargetController::NotYou,
                        permanent_types,
                        ..
                    } if permanent_types == &[PermanentTypeFilter::Creature, PermanentTypeFilter::Planeswalker]
                ),
                "{id}: creature or planeswalker you don't control"
            ),
            _ => assert!(
                matches!(
                    target,
                    tricerules_cards::primitives::ObjectFilter {
                        kind: TargetKind::Creature,
                        controller: tricerules_cards::primitives::TargetController::NotYou,
                        ..
                    }
                ),
                "{id}: creature you don't control"
            ),
        }
        let targeting = f.targeting.as_ref().expect("two target groups");
        assert_eq!(targeting.groups.len(), 2, "{id}");
        assert_eq!(targeting.groups[0].max, 1, "{id}");
        assert_eq!(targeting.groups[1].max, 1, "{id}");
        assert_eq!(
            targeting.groups[1].distinct_from,
            vec![0],
            "{id}: roles are distinct"
        );
        assert!(distinct);
        assert_eq!(targeting.groups[1].effect_indices, vec![0], "{id}");
        // The second group is the opposing creature/planeswalker; the first is your creature.
        let _ = (target_kind, target_types);
    }
}

// ── fingerprints ────────────────────────────────────────────────────────────

#[test]
fn q47_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name) in [
        ("vinelasher_kudzu", "Vinelasher Kudzu"),
        ("xindi_surveyors", "Xindi Surveyors"),
        ("prowling_felidar", "Prowling Felidar"),
        ("territorial_scythecat", "Territorial Scythecat"),
        ("deeproot_champion", "Deeproot Champion"),
        ("pyroceratops", "Pyroceratops"),
        ("sprite_dragon", "Sprite Dragon"),
        ("faerie_vandal", "Faerie Vandal"),
        ("knights_of_dol_amroth", "Knights of Dol Amroth"),
        ("thopter_mechanic", "Thopter Mechanic"),
        ("bloodcrusher_of_khorne", "Bloodcrusher of Khorne"),
        ("khenra_charioteer", "Khenra Charioteer"),
        ("nyleas_forerunner", "Nylea's Forerunner"),
        ("absorb_vis", "Absorb Vis"),
        ("fiery_fall", "Fiery Fall"),
        ("mental_journey", "Mental Journey"),
        ("sylvan_bounty", "Sylvan Bounty"),
        ("power_of_fire", "Power of Fire"),
        ("psionic_gift", "Psionic Gift"),
        ("fire_whip", "Fire Whip"),
        ("desert_sandstorm", "Desert Sandstorm"),
        ("sweltering_suns", "Sweltering Suns"),
        ("seismic_shudder", "Seismic Shudder"),
        ("tremor", "Tremor"),
        ("defeat", "Defeat"),
        ("smite_the_monstrous", "Smite the Monstrous"),
        ("neck_snap", "Neck Snap"),
        ("daring_demolition", "Daring Demolition"),
        ("dreadbore", "Dreadbore"),
        ("aggressive_instinct", "Aggressive Instinct"),
        ("masters_rebuke", "Master's Rebuke"),
        ("tail_slash", "Tail Slash"),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "{id}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id, "face id drift for {id}");
        assert_eq!(fields[3], name);
        assert_eq!(fields[4].len(), 64, "fingerprint length for {id}");
        assert!(
            fields[4].chars().all(|ch| ch.is_ascii_hexdigit()),
            "fingerprint digest for {id}"
        );
        assert_eq!(definition(id).primary_face().name, name, "{id}");
    }
}
