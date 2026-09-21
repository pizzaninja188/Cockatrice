//! Registry conformance for the second issue #338 reviewed direct-RON cohort.
//!
//! Fanatical Offering (`3e5402bf-ce7b-4c23-9333-0acdcf349caf`), Betrayer's Bargain
//! (`bfb3d862-d92d-4a4f-9a22-671b55d954fd`), Dusk Rose Reliquary
//! (`dca7102e-c020-4fdc-9b59-f186e3e615a5`), Mudbutton Cursetosser
//! (`b63e5518-61f3-4cde-a799-5f3ee6aba70b`) and Lys Alana Dignitary
//! (`ce7582a2-e014-41fb-8753-00d29cffc81a`) were promoted after complete-definition review against
//! the pinned Scryfall snapshot. The exact records and `rulings_uri` were fetched 2026-09-20.
//! Governance: CR 118.8/601.2b/f-h (announced additional costs and total cost), 701.4 (behold),
//! 701.16 (discard), 701.9 (sacrifice), 121.1 (draw), 111.10s (Map), 120.3/616.1 (damage and the
//! exile-if-would-die replacement), 702.21 (Ward), 610.3 (linked exile), 508.1c/509.1b (combat
//! restrictions), 603.6a/603.7 (dies and entry triggers), 106.1/605.1a (mana abilities), and
//! 608.2c (printed instruction order).

use tricerules_cards::primitives::{
    AbilityCost, Amount, CastCostGroupDef, CastCostOptionDef, CombatRestriction, EffectSubject,
    GameCondition, GraveyardAggregate, ManaCostChoiceKind, ObjectCastCostKind, PermanentTypeFilter,
    PlayerRecipient, RelativePlayerSet, ResolutionCost, SpellEffectKind, StaticAbilityDef,
    TargetController, TargetFilter, TargetGroupDef, TargetKind, TargetingDef,
    TargetingSourceFilter, TriggerCondition, ZoneCardFilter,
};
use tricerules_cards::{
    AbilityPresentation, CardRegistry, CastTriggerPlayer, ChoiceId, Color, Layout,
};

const FANATICAL_OFFERING_FINGERPRINT: &str =
    "ee269ff8f501988afe34a6477933bdaac72133a43de77710ea07ce30565cd210";
const BETRAYERS_BARGAIN_FINGERPRINT: &str =
    "fa8652e671eafb04f167451c3765012fc31310d79faecf8604296555f38a3b0a";
const DUSK_ROSE_RELIQUARY_FINGERPRINT: &str =
    "e3c37492745fcc08480af22aa052b0ce6b0f9924cf2b595c77f75d9fa00af516";
const MUDBUTTON_CURSETOSSER_FINGERPRINT: &str =
    "ee0e1c9a4b42a6aff9b5db403a97909a078a4d2b353e794d805567b8721a75b0";
const LYS_ALANA_DIGNITARY_FINGERPRINT: &str =
    "72655eb57dbf4099d448313378e3b668ba2ad5e773858827a89a3e5c94391b1a";

fn single_group(targeting: &TargetingDef) -> &TargetGroupDef {
    let [group] = targeting.groups.as_slice() else {
        panic!("expected exactly one authored target group");
    };
    group
}

fn you_artifact() -> TargetFilter {
    TargetFilter {
        kind: TargetKind::AnyPermanent,
        controller: TargetController::You,
        permanent_types: vec![PermanentTypeFilter::Artifact],
        ..TargetFilter::default()
    }
}

fn you_creature() -> TargetFilter {
    TargetFilter {
        kind: TargetKind::Creature,
        controller: TargetController::You,
        ..TargetFilter::default()
    }
}

fn you_enchantment() -> TargetFilter {
    TargetFilter {
        kind: TargetKind::AnyPermanent,
        controller: TargetController::You,
        permanent_types: vec![PermanentTypeFilter::Enchantment],
        ..TargetFilter::default()
    }
}

fn artifact_or_creature_sacrifice(group: &CastCostGroupDef) -> &TargetFilter {
    let CastCostOptionDef::SacrificePermanent {
        option_id,
        kind,
        filter,
        ..
    } = &group.options[0]
    else {
        panic!("expected the artifact-or-creature sacrifice option first");
    };
    assert_eq!(
        *option_id,
        ChoiceId::new("sacrifice_artifact_or_creature").unwrap()
    );
    assert_eq!(*kind, ObjectCastCostKind::AdditionalPayment);
    filter.as_ref()
}

fn mana_option_cost(group: &CastCostGroupDef, index: usize) -> String {
    let CastCostOptionDef::Mana { cost, kind, .. } = &group.options[index] else {
        panic!("expected a mana option at index {index}");
    };
    assert_eq!(*kind, ManaCostChoiceKind::AdditionalPayment);
    cost.to_string()
}

fn behold_option(
    group: &CastCostGroupDef,
    index: usize,
) -> (&ChoiceId, &ZoneCardFilter, &TargetFilter) {
    let CastCostOptionDef::Behold {
        option_id,
        hand_filter,
        permanent_filter,
        ..
    } = &group.options[index]
    else {
        panic!("expected a behold option at index {index}");
    };
    (option_id, hand_filter, permanent_filter.as_ref())
}

#[test]
fn issue_338_direct_ron_batch2_maps_definitions() {
    let registry = CardRegistry::global();

    // Fanatical Offering: sacrifice an artifact or creature, then draw two and make a Map.
    let offering = registry.get("fanatical_offering").expect("registered");
    assert_eq!(offering.name, "Fanatical Offering");
    assert_eq!(
        registry.id_for_name("Fanatical Offering"),
        Some("fanatical_offering")
    );
    assert_eq!(offering.layout, Layout::Normal);
    assert_eq!(offering.face_count(), 1);
    let face = offering.primary_face();
    assert_eq!(face.face_id.as_str(), "fanatical_offering");
    assert_eq!(face.mana_cost.to_string(), "{1}{B}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::Black]);
    {
        let [group] = face.cast_cost_groups.as_slice() else {
            panic!("Fanatical Offering has exactly one cast-cost group");
        };
        assert_eq!(group.group_id, ChoiceId::new("additional_cost").unwrap());
        assert_eq!((group.min, group.max), (1, 1));
        assert_eq!(
            group.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(
            artifact_or_creature_sacrifice(group),
            &TargetFilter {
                any_of: Some(vec![you_artifact(), you_creature()]),
                ..TargetFilter::default()
            }
        );
    }
    assert_eq!(
        face.spell_effect,
        [
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(2),
            },
            SpellEffectKind::CreateTokens {
                token: "map".into(),
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            },
        ]
    );
    assert!(face.targeting.is_none());
    assert!(registry.is_token("map"));

    // Betrayer's Bargain: sacrifice a creature/enchantment or pay {2}, then damage and a
    // turn-long exile replacement on the same target.
    let bargain = registry.get("betrayers_bargain").expect("registered");
    assert_eq!(bargain.name, "Betrayer's Bargain");
    assert_eq!(
        registry.id_for_name("Betrayer's Bargain"),
        Some("betrayers_bargain")
    );
    let face = bargain.primary_face();
    assert_eq!(face.face_id.as_str(), "betrayers_bargain");
    assert_eq!(face.mana_cost.to_string(), "{1}{R}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::Red]);
    {
        let [group] = face.cast_cost_groups.as_slice() else {
            panic!("Betrayer's Bargain has exactly one cast-cost group");
        };
        assert_eq!((group.min, group.max), (1, 1));
        let CastCostOptionDef::SacrificePermanent {
            option_id,
            kind,
            filter,
            ..
        } = &group.options[0]
        else {
            panic!("Betrayer's Bargain sacrifices first");
        };
        assert_eq!(
            *option_id,
            ChoiceId::new("sacrifice_creature_or_enchantment").unwrap()
        );
        assert_eq!(*kind, ObjectCastCostKind::AdditionalPayment);
        assert_eq!(
            filter.as_ref(),
            &TargetFilter {
                any_of: Some(vec![you_creature(), you_enchantment()]),
                ..TargetFilter::default()
            }
        );
        assert_eq!(mana_option_cost(group, 1), "{2}");
    }
    assert_eq!(
        face.spell_effect,
        [
            SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(5),
                target: TargetFilter {
                    kind: TargetKind::Creature,
                    ..TargetFilter::default()
                },
            },
            SpellEffectKind::ExileIfWouldDieThisTurn {
                target: TargetFilter {
                    kind: TargetKind::Creature,
                    ..TargetFilter::default()
                },
            },
        ]
    );
    let targeting = face.targeting.as_ref().expect("Betrayer's Bargain targets");
    let group = single_group(targeting);
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target creature");
    assert_eq!(group.effect_indices, [0, 1]);

    // Dusk Rose Reliquary: sacrifice an artifact or creature, Ward {2}, and a linked exile.
    let reliquary = registry.get("dusk_rose_reliquary").expect("registered");
    assert_eq!(reliquary.name, "Dusk Rose Reliquary");
    assert_eq!(
        registry.id_for_name("Dusk Rose Reliquary"),
        Some("dusk_rose_reliquary")
    );
    let face = reliquary.primary_face();
    assert_eq!(face.face_id.as_str(), "dusk_rose_reliquary");
    assert_eq!(face.mana_cost.to_string(), "{W}");
    assert_eq!(face.types, ["Artifact"]);
    assert_eq!(face.colors(), vec![Color::White]);
    {
        let [group] = face.cast_cost_groups.as_slice() else {
            panic!("Dusk Rose Reliquary has exactly one cast-cost group");
        };
        assert_eq!(
            artifact_or_creature_sacrifice(group),
            &TargetFilter {
                any_of: Some(vec![you_artifact(), you_creature()]),
                ..TargetFilter::default()
            }
        );
    }
    let [ward, entry] = face.triggered_abilities.as_slice() else {
        panic!("Dusk Rose Reliquary has exactly the Ward and entry triggers");
    };
    assert_eq!(ward.ability_id.as_str(), "triggered_01");
    assert_eq!(ward.presentation, AbilityPresentation::OracleLines(vec![2]));
    assert_eq!(
        ward.trigger,
        TriggerCondition::WheneverSelfBecomesTarget {
            source: TargetingSourceFilter::SpellOrAbility,
            source_controller: CastTriggerPlayer::Opponent,
        }
    );
    assert_eq!(ward.effect.len(), 1);
    match &ward.effect[0] {
        SpellEffectKind::CounterTriggeringStackObjectUnlessPays {
            cost: ResolutionCost::Mana(cost),
        } => assert_eq!(cost.to_string(), "{2}"),
        other => panic!("Ward {{2}} must be a mana payment, got {other:?}"),
    }
    assert!(ward.targeting.is_none());

    assert_eq!(entry.ability_id.as_str(), "triggered_02");
    assert_eq!(
        entry.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(entry.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        entry.effect,
        [SpellEffectKind::ExileUntilSourceLeaves {
            target: TargetFilter {
                kind: TargetKind::AnyPermanent,
                controller: TargetController::Opponent,
                permanent_types: vec![PermanentTypeFilter::Artifact, PermanentTypeFilter::Creature],
                ..TargetFilter::default()
            },
        }]
    );
    let targeting = entry.targeting.as_ref().expect("linked exile targets");
    let group = single_group(targeting);
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(
        group.prompt,
        "Choose target artifact or creature an opponent controls"
    );
    assert_eq!(group.effect_indices, [0]);

    // Mudbutton Cursetosser: behold a Goblin or pay {2}, cannot block, and punishes on death.
    let cursetosser = registry.get("mudbutton_cursetosser").expect("registered");
    assert_eq!(cursetosser.name, "Mudbutton Cursetosser");
    assert_eq!(
        registry.id_for_name("Mudbutton Cursetosser"),
        Some("mudbutton_cursetosser")
    );
    let face = cursetosser.primary_face();
    assert_eq!(face.face_id.as_str(), "mudbutton_cursetosser");
    assert_eq!(face.mana_cost.to_string(), "{B}");
    assert_eq!(face.types, ["Creature", "Goblin", "Warlock"]);
    assert_eq!(face.colors(), vec![Color::Black]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(1)));
    {
        let [group] = face.cast_cost_groups.as_slice() else {
            panic!("Mudbutton Cursetosser has exactly one cast-cost group");
        };
        let (option_id, hand_filter, permanent_filter) = behold_option(group, 0);
        assert_eq!(*option_id, ChoiceId::new("behold").unwrap());
        assert_eq!(hand_filter.required_subtypes, ["Goblin"]);
        assert_eq!(
            permanent_filter,
            &TargetFilter {
                kind: TargetKind::AnyPermanent,
                controller: TargetController::You,
                required_subtypes: vec!["Goblin".into()],
                ..TargetFilter::default()
            }
        );
        assert_eq!(mana_option_cost(group, 1), "{2}");
    }
    assert_eq!(
        face.static_abilities,
        [StaticAbilityDef::SelfCombatRestriction {
            restriction: CombatRestriction {
                cant_block: true,
                ..CombatRestriction::default()
            },
            condition: None,
        }]
    );
    let [dies] = face.triggered_abilities.as_slice() else {
        panic!("Mudbutton Cursetosser has exactly one dies trigger");
    };
    assert_eq!(dies.ability_id.as_str(), "triggered_01");
    assert_eq!(dies.presentation, AbilityPresentation::OracleLines(vec![3]));
    assert_eq!(dies.trigger, TriggerCondition::WhenSelfDies);
    assert_eq!(
        dies.effect,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::Opponent,
                power: Some(tricerules_cards::primitives::PowerComparison::AtMost(2)),
                ..TargetFilter::default()
            })),
        }]
    );
    let targeting = dies.targeting.as_ref().expect("dies trigger targets");
    let group = single_group(targeting);
    assert_eq!(
        group.prompt,
        "Choose target creature an opponent controls with power 2 or less"
    );

    // Lys Alana Dignitary: behold an Elf or pay {2}, and a conditional {G}{G} mana ability.
    let dignitary = registry.get("lys_alana_dignitary").expect("registered");
    assert_eq!(dignitary.name, "Lys Alana Dignitary");
    assert_eq!(
        registry.id_for_name("Lys Alana Dignitary"),
        Some("lys_alana_dignitary")
    );
    let face = dignitary.primary_face();
    assert_eq!(face.face_id.as_str(), "lys_alana_dignitary");
    assert_eq!(face.mana_cost.to_string(), "{1}{G}");
    assert_eq!(face.types, ["Creature", "Elf", "Advisor"]);
    assert_eq!(face.colors(), vec![Color::Green]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(3)));
    {
        let [group] = face.cast_cost_groups.as_slice() else {
            panic!("Lys Alana Dignitary has exactly one cast-cost group");
        };
        let (option_id, hand_filter, permanent_filter) = behold_option(group, 0);
        assert_eq!(*option_id, ChoiceId::new("behold").unwrap());
        assert_eq!(hand_filter.required_subtypes, ["Elf"]);
        assert_eq!(
            permanent_filter,
            &TargetFilter {
                kind: TargetKind::AnyPermanent,
                controller: TargetController::You,
                required_subtypes: vec!["Elf".into()],
                ..TargetFilter::default()
            }
        );
        assert_eq!(mana_option_cost(group, 1), "{2}");
    }
    let [mana] = face.activated_abilities.as_slice() else {
        panic!("Lys Alana Dignitary has exactly one activated ability");
    };
    assert_eq!(mana.ability_id.as_str(), "activated_01");
    assert_eq!(mana.presentation, AbilityPresentation::OracleLines(vec![2]));
    assert_eq!(mana.costs, [AbilityCost::Tap]);
    assert_eq!(
        mana.conditions,
        [GameCondition::GraveyardAggregate {
            owners: RelativePlayerSet::Controller,
            aggregate: GraveyardAggregate::CardCount,
            filter: Some(ZoneCardFilter {
                required_subtypes: vec!["Elf".into()],
                ..ZoneCardFilter::default()
            }),
            min: Some(1),
            max: None,
        }]
    );
    assert_eq!(mana.effect.len(), 1);
    match &mana.effect[0] {
        SpellEffectKind::ProduceMana { options, .. } => {
            assert_eq!(options.len(), 1);
            assert_eq!(options[0].g, 2);
            assert_eq!(
                options[0].w + options[0].u + options[0].b + options[0].r + options[0].c,
                0
            );
        }
        other => panic!("expected a green mana ability, got {other:?}"),
    }
    assert!(mana.targeting.is_none());
}

#[test]
fn issue_338_direct_ron_batch2_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, expected) in [
        (
            "fanatical_offering",
            "Fanatical Offering",
            FANATICAL_OFFERING_FINGERPRINT,
        ),
        (
            "betrayers_bargain",
            "Betrayer's Bargain",
            BETRAYERS_BARGAIN_FINGERPRINT,
        ),
        (
            "dusk_rose_reliquary",
            "Dusk Rose Reliquary",
            DUSK_ROSE_RELIQUARY_FINGERPRINT,
        ),
        (
            "mudbutton_cursetosser",
            "Mudbutton Cursetosser",
            MUDBUTTON_CURSETOSSER_FINGERPRINT,
        ),
        (
            "lys_alana_dignitary",
            "Lys Alana Dignitary",
            LYS_ALANA_DIGNITARY_FINGERPRINT,
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
        assert_eq!(fields[2], id);
        assert_eq!(fields[3], name);
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
