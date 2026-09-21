//! Registry conformance for the issue #359 Spree cohort plus the issue #338 Pyrrhic Strike.
//!
//! Three Steps Ahead (`282dfeaa-6243-4f92-838a-5cb54fa85184`), Trash the Town
//! (`b47f7f1e-2d81-4b1b-870d-4c1a4f020505`), Insatiable Avarice
//! (`ad3e705f-da57-4eff-84d7-2072522de988`), Jailbreak Scheme
//! (`bc7f39e1-97bf-4725-a84a-2a04121530c6`) and Pyrrhic Strike
//! (`5ea1d89c-8650-4373-9835-e369587020ef`) were promoted after complete-definition review against
//! the pinned Scryfall snapshot. The exact records and `rulings_uri` were fetched 2026-09-21.
//! Governance: CR 702.171a-e (spree), 601.2b/f-h (announced additional costs), 700.2 (mode
//! announcement and linked choices), 115.1/608.2b (targeting and revalidation), 701.7 (destroy),
//! 701.30/118.8 (blight), 121.1 (draw), 122.1 (counters), 509.1b/611.3 (combat restrictions),
//! 611.2c (granted triggered abilities), and 400.7 (new-object identity for token copies).

use tricerules_cards::primitives::{
    Amount, CastCostGroupDef, CastCostOptionDef, CombatRestriction, CombatRestrictionScope,
    DrawDiscardOrder, EffectSubject, LibraryPlacement, ManaCostChoiceKind, PermanentTypeFilter,
    PlayerRecipient, SearchDestination, SpellEffectKind, StackSpellFilter, TargetController,
    TargetFilter, TargetKind, TokenCopySource, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, ChoiceId, Color, Layout, ModalDef};

const THREE_STEPS_AHEAD_FINGERPRINT: &str =
    "a87a5977e3eae7b0288c2a3b5fac03b84a7e500c6bc4026dd011c39939982472";
const TRASH_THE_TOWN_FINGERPRINT: &str =
    "cc96818136cc1f2f1ccbb2be9ebaf7b132aeb7e592e8072269239863e07ae847";
const INSATIABLE_AVARICE_FINGERPRINT: &str =
    "78f10fa32d688f0fbed7a1891e2ce1896c53df35635810d494da59fd2dd30105";
const JAILBREAK_SCHEME_FINGERPRINT: &str =
    "377a4c36e08adc570a0b5bd17fec68671852b95d2fd80cbe84b16d282f643a2d";
const PYRRHIC_STRIKE_FINGERPRINT: &str =
    "a81ed4b39639cfe532d8083f14bee9453599c09a69723079edf79ccfd20589d5";

fn single_group(
    targeting: &tricerules_cards::primitives::TargetingDef,
) -> &tricerules_cards::primitives::TargetGroupDef {
    let [group] = targeting.groups.as_slice() else {
        panic!("expected exactly one authored target group");
    };
    group
}

fn spree_group(face: &tricerules_cards::CardFace) -> &CastCostGroupDef {
    let [group] = face.cast_cost_groups.as_slice() else {
        panic!("expected exactly one spree cast-cost group");
    };
    assert_eq!(group.group_id, ChoiceId::new("spree").unwrap());
    group
}

fn mana_cost(group: &CastCostGroupDef, index: usize) -> String {
    let CastCostOptionDef::Mana { kind, cost, .. } = &group.options[index] else {
        panic!("expected a mana option at index {index}");
    };
    assert_eq!(*kind, ManaCostChoiceKind::AdditionalPayment);
    cost.to_string()
}

fn modal_of(face: &tricerules_cards::CardFace) -> &ModalDef {
    face.modal_spell.as_ref().expect("Spree is modal")
}

fn counter_spell_default() -> SpellEffectKind {
    SpellEffectKind::CounterTargetSpell {
        spell_filter: StackSpellFilter::default(),
        unless_controller_pays: None,
        unless_controller_pays_by_cast_cost: None,
    }
}

#[test]
fn issue_359_direct_ron_batch2_maps_definitions() {
    let registry = CardRegistry::global();

    // Three Steps Ahead: {U} spree; counter / copy / draw-discard linked to {1}{U}, {3}, {2}.
    let three = registry.get("three_steps_ahead").expect("registered");
    assert_eq!(three.name, "Three Steps Ahead");
    assert_eq!(
        registry.id_for_name("Three Steps Ahead"),
        Some("three_steps_ahead")
    );
    assert_eq!(three.layout, Layout::Normal);
    let face = three.primary_face();
    assert_eq!(face.face_id.as_str(), "three_steps_ahead");
    assert_eq!(face.mana_cost.to_string(), "{U}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::Blue]);
    let group = spree_group(face);
    assert_eq!((group.min, group.max), (1, 3));
    assert_eq!(mana_cost(group, 0), "{1}{U}");
    assert_eq!(mana_cost(group, 1), "{3}");
    assert_eq!(mana_cost(group, 2), "{2}");
    let modal = modal_of(face);
    assert_eq!((modal.min_modes, modal.max_modes), (1, 3));
    assert!(modal.all_modes_cast_cost.is_none());
    let [counter, copy, draw] = modal.modes.as_slice() else {
        panic!("Three Steps Ahead has three modes");
    };
    assert_eq!(
        counter.mode_id,
        tricerules_cards::ModeId::new("counter").unwrap()
    );
    assert_eq!(
        counter.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(counter.effects, [counter_spell_default()]);
    assert_eq!(
        single_group(counter.targeting.as_ref().unwrap()).prompt,
        "Choose target spell"
    );
    assert_eq!(copy.mode_id, tricerules_cards::ModeId::new("copy").unwrap());
    assert_eq!(
        copy.effects,
        [SpellEffectKind::CreateTokenCopies {
            count: Amount::Fixed(1),
            source: TokenCopySource::Chosen(Box::new(TargetFilter {
                any_of: Some(vec![
                    TargetFilter {
                        kind: TargetKind::AnyPermanent,
                        controller: TargetController::You,
                        permanent_types: vec![PermanentTypeFilter::Artifact],
                        ..TargetFilter::default()
                    },
                    TargetFilter {
                        kind: TargetKind::Creature,
                        controller: TargetController::You,
                        ..TargetFilter::default()
                    },
                ]),
                ..TargetFilter::default()
            })),
        }]
    );
    assert_eq!(
        single_group(copy.targeting.as_ref().unwrap()).prompt,
        "Choose target artifact or creature you control"
    );
    assert_eq!(
        draw.mode_id,
        tricerules_cards::ModeId::new("draw_discard").unwrap()
    );
    assert_eq!(
        draw.effects,
        [SpellEffectKind::DrawDiscard {
            who: PlayerRecipient::Controller,
            draw_count: 2,
            discard_count: 1,
            order: DrawDiscardOrder::DrawThenDiscard,
            optional: false,
        }]
    );
    assert!(draw.targeting.is_none());

    // Trash the Town: counters / trample / granted combat-damage draw, linked to {2}, {1}, {1}.
    let trash = registry.get("trash_the_town").expect("registered");
    assert_eq!(trash.name, "Trash the Town");
    let face = trash.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{G}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::Green]);
    let group = spree_group(face);
    assert_eq!((group.min, group.max), (1, 3));
    assert_eq!(mana_cost(group, 0), "{2}");
    assert_eq!(mana_cost(group, 1), "{1}");
    assert_eq!(mana_cost(group, 2), "{1}");
    let modal = modal_of(face);
    assert_eq!((modal.min_modes, modal.max_modes), (1, 3));
    let [counters, trample, grant] = modal.modes.as_slice() else {
        panic!("Trash the Town has three modes");
    };
    assert_eq!(
        counters.effects,
        [SpellEffectKind::PutCounters {
            counter: tricerules_cards::CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(2),
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                ..TargetFilter::default()
            })),
        }]
    );
    assert_eq!(
        trample.effects,
        [SpellEffectKind::GrantKeywords {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                ..TargetFilter::default()
            })),
            keywords: vec![tricerules_cards::Keyword::Trample],
        }]
    );
    let [granted] = grant.effects.as_slice() else {
        panic!("grant mode has one effect");
    };
    let SpellEffectKind::GrantTriggeredAbility { subject, ability } = granted else {
        panic!("grant mode grants a triggered ability, got {granted:?}");
    };
    assert_eq!(
        *subject,
        EffectSubject::Chosen(Box::new(TargetFilter {
            kind: TargetKind::Creature,
            ..TargetFilter::default()
        }))
    );
    assert_eq!(
        ability.trigger,
        TriggerCondition::WheneverSelfDealsCombatDamageToPlayer
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(2),
        }]
    );

    // Insatiable Avarice: tutor-to-top and target-player draw/drain, linked to {2} and {B}{B}.
    let avarice = registry.get("insatiable_avarice").expect("registered");
    assert_eq!(avarice.name, "Insatiable Avarice");
    let face = avarice.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{B}");
    assert_eq!(face.types, ["Sorcery"]);
    let group = spree_group(face);
    assert_eq!((group.min, group.max), (1, 2));
    assert_eq!(mana_cost(group, 0), "{2}");
    assert_eq!(mana_cost(group, 1), "{B}{B}");
    let modal = modal_of(face);
    assert_eq!((modal.min_modes, modal.max_modes), (1, 2));
    let [tutor, draw_drain] = modal.modes.as_slice() else {
        panic!("Insatiable Avarice has two modes");
    };
    let [tutor_effect] = tutor.effects.as_slice() else {
        panic!("tutor mode has one effect");
    };
    let SpellEffectKind::SearchLibrary {
        destination,
        count,
        shuffle,
        ..
    } = tutor_effect
    else {
        panic!("tutor mode searches, got {tutor_effect:?}");
    };
    assert_eq!(*destination, SearchDestination::TopOfLibrary);
    assert_eq!(*count, 1);
    assert!(*shuffle);
    assert!(tutor.targeting.is_none());
    assert_eq!(
        draw_drain.effects,
        [
            SpellEffectKind::TargetPlayerDraws {
                count: 3,
                target: TargetFilter {
                    kind: TargetKind::AnyPlayer,
                    ..TargetFilter::default()
                },
            },
            SpellEffectKind::TargetPlayerLosesLife {
                amount: 3,
                target: TargetFilter {
                    kind: TargetKind::AnyPlayer,
                    ..TargetFilter::default()
                },
            },
        ]
    );
    assert_eq!(
        single_group(draw_drain.targeting.as_ref().unwrap()).effect_indices,
        [0, 1]
    );

    // Jailbreak Scheme: counter+unblockable and owner-choice library placement.
    let jailbreak = registry.get("jailbreak_scheme").expect("registered");
    assert_eq!(jailbreak.name, "Jailbreak Scheme");
    let face = jailbreak.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{U}");
    assert_eq!(face.types, ["Sorcery"]);
    let group = spree_group(face);
    assert_eq!((group.min, group.max), (1, 2));
    assert_eq!(mana_cost(group, 0), "{3}");
    assert_eq!(mana_cost(group, 1), "{2}");
    let modal = modal_of(face);
    let [counter_block, library_choice] = modal.modes.as_slice() else {
        panic!("Jailbreak Scheme has two modes");
    };
    assert_eq!(
        counter_block.effects,
        [
            SpellEffectKind::PutCounters {
                counter: tricerules_cards::CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                subject: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::Creature,
                    ..TargetFilter::default()
                })),
            },
            SpellEffectKind::ApplyCombatRestriction {
                scope: CombatRestrictionScope::Chosen(TargetFilter {
                    kind: TargetKind::Creature,
                    ..TargetFilter::default()
                }),
                restriction: CombatRestriction {
                    cant_be_blocked: true,
                    ..CombatRestriction::default()
                },
            },
        ]
    );
    assert_eq!(
        single_group(counter_block.targeting.as_ref().unwrap()).effect_indices,
        [0, 1]
    );
    assert_eq!(
        library_choice.effects,
        [SpellEffectKind::PutInOwnersLibrary {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::AnyPermanent,
                permanent_types: vec![PermanentTypeFilter::Artifact, PermanentTypeFilter::Creature],
                ..TargetFilter::default()
            })),
            placement: LibraryPlacement::OwnerChoiceTopOrBottom,
        }]
    );

    // Pyrrhic Strike: optional blight 2 unlocks both destroy modes.
    let pyrrhic = registry.get("pyrrhic_strike").expect("registered");
    assert_eq!(pyrrhic.name, "Pyrrhic Strike");
    let face = pyrrhic.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{2}{W}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::White]);
    let [group] = face.cast_cost_groups.as_slice() else {
        panic!("Pyrrhic Strike has one cast-cost group");
    };
    assert_eq!(group.group_id, ChoiceId::new("additional_cost").unwrap());
    assert_eq!((group.min, group.max), (0, 1));
    let CastCostOptionDef::Blight {
        option_id, count, ..
    } = &group.options[0]
    else {
        panic!("Pyrrhic Strike's option is blight");
    };
    assert_eq!(*option_id, ChoiceId::new("blight").unwrap());
    assert_eq!(*count, 2);
    assert_eq!(group.options[0].fallback_label(), "Blight 2".to_string());
    let modal = modal_of(face);
    assert_eq!((modal.min_modes, modal.max_modes), (1, 2));
    assert_eq!(
        modal.all_modes_cast_cost,
        Some(tricerules_cards::primitives::CastCostOptionRef {
            group_id: ChoiceId::new("additional_cost").unwrap(),
            option_id: ChoiceId::new("blight").unwrap(),
        })
    );
    let [artifact, creature] = modal.modes.as_slice() else {
        panic!("Pyrrhic Strike has two modes");
    };
    assert_eq!(
        artifact.effects,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                any_of: Some(vec![
                    TargetFilter {
                        kind: TargetKind::AnyPermanent,
                        permanent_types: vec![PermanentTypeFilter::Artifact],
                        ..TargetFilter::default()
                    },
                    TargetFilter {
                        kind: TargetKind::AnyPermanent,
                        permanent_types: vec![PermanentTypeFilter::Enchantment],
                        ..TargetFilter::default()
                    },
                ]),
                ..TargetFilter::default()
            })),
        }]
    );
    assert_eq!(
        creature.effects,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                min_mana_value: Some(3),
                ..TargetFilter::default()
            })),
        }]
    );
}

#[test]
fn issue_359_direct_ron_batch2_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, expected) in [
        (
            "three_steps_ahead",
            "Three Steps Ahead",
            THREE_STEPS_AHEAD_FINGERPRINT,
        ),
        (
            "trash_the_town",
            "Trash the Town",
            TRASH_THE_TOWN_FINGERPRINT,
        ),
        (
            "insatiable_avarice",
            "Insatiable Avarice",
            INSATIABLE_AVARICE_FINGERPRINT,
        ),
        (
            "jailbreak_scheme",
            "Jailbreak Scheme",
            JAILBREAK_SCHEME_FINGERPRINT,
        ),
        (
            "pyrrhic_strike",
            "Pyrrhic Strike",
            PYRRHIC_STRIKE_FINGERPRINT,
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
