//! Registry conformance for the queue-v49 reviewed cohorts (batch C).
#![allow(clippy::type_complexity)]
use tricerules_cards::primitives::{
    AbilityCost, Amount, CardTypeFilter, CounterKind, GameCondition, GraveyardDestination,
    GraveyardOwner, HandCardAction, ManaAmount, RelativePlayerSet, SpellEffectKind,
    StaticAbilityDef, TriggerCondition,
};
use tricerules_cards::{registry, AbilityPresentation, CardDefinition, CardFace, Layout};

fn definition(id: &str) -> &'static CardDefinition {
    registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
}

fn face_id(name: &str) -> String {
    let folded = name.to_lowercase().replace(['\'', ','], "");
    let mut text = String::new();
    let mut last_underscore = false;
    for ch in folded.chars() {
        if ch.is_ascii_alphanumeric() {
            text.push(ch);
            last_underscore = false;
        } else if !last_underscore {
            text.push('_');
            last_underscore = true;
        }
    }
    text.trim_matches('_').to_string()
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
    assert_eq!(f.face_id.as_str(), face_id(name), "{id} face id");
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

// ── 06 Graveyard to hand ETB ───────────────────────────────────────────────

#[test]
fn q49_graveyard_hand_maps_definitions() {
    for (id, name, mana, body, types) in [
        (
            "archaeomender",
            "Archaeomender",
            "{2}{U}",
            (2, 3),
            &["Creature", "Human", "Wizard"][..],
        ),
        (
            "barrow_witches",
            "Barrow Witches",
            "{4}{B}",
            (3, 4),
            &["Creature", "Human", "Warlock"][..],
        ),
        (
            "golgari_findbroker",
            "Golgari Findbroker",
            "{B}{B}{G}{G}",
            (3, 4),
            &["Creature", "Elf", "Shaman"][..],
        ),
        (
            "monk_idealist",
            "Monk Idealist",
            "{2}{W}",
            (2, 2),
            &["Creature", "Human", "Monk", "Cleric"][..],
        ),
        (
            "restoration_gearsmith",
            "Restoration Gearsmith",
            "{2}{W}{B}",
            (3, 3),
            &["Creature", "Human", "Artificer"][..],
        ),
        (
            "warden_of_the_eye",
            "Warden of the Eye",
            "{2}{U}{R}{W}",
            (3, 3),
            &["Creature", "Djinn", "Wizard"][..],
        ),
    ] {
        let f = identity(id, name, mana, types, Some(body));
        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{id}: one enters trigger");
        };
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert_eq!(
            trigger.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        let [SpellEffectKind::MoveGraveyardCards {
            filter,
            destination,
            ..
        }] = trigger.effect.as_slice()
        else {
            panic!("{id}: MoveGraveyardCards");
        };
        assert_eq!(*destination, GraveyardDestination::Hand, "{id}");
        assert_eq!(filter.owner, GraveyardOwner::Controller, "{id}");
        let card = filter
            .card
            .as_ref()
            .unwrap_or_else(|| panic!("{id}: card filter"));
        match id {
            "archaeomender" => {
                assert_eq!(card.card_type, Some(CardTypeFilter::Artifact), "{id}");
            }
            "barrow_witches" => {
                assert_eq!(card.required_subtypes, ["Knight"], "{id}");
            }
            "golgari_findbroker" => {
                let any = card.any_of.as_ref().expect("permanent alternatives");
                assert_eq!(any.len(), 2, "{id}");
                assert_eq!(any[0].card_type, Some(CardTypeFilter::Land), "{id}");
                assert_eq!(
                    any[1].card_type,
                    Some(CardTypeFilter::NonlandPermanent),
                    "{id}"
                );
            }
            "monk_idealist" => {
                assert_eq!(card.card_type, Some(CardTypeFilter::Enchantment), "{id}");
            }
            "restoration_gearsmith" => {
                let any = card.any_of.as_ref().expect("artifact or creature");
                assert_eq!(any.len(), 2, "{id}");
                assert_eq!(any[0].card_type, Some(CardTypeFilter::Artifact), "{id}");
                assert_eq!(any[1].card_type, Some(CardTypeFilter::Creature), "{id}");
            }
            "warden_of_the_eye" => {
                assert!(card.excluded_card_types.contains(&CardTypeFilter::Creature));
                assert!(card.excluded_card_types.contains(&CardTypeFilter::Land));
            }
            other => panic!("unhandled {other}"),
        }
    }
}

// ── 07 Sacrifice for mana ──────────────────────────────────────────────────

#[test]
fn q49_sacrifice_mana_maps_definitions() {
    struct Case {
        id: &'static str,
        name: &'static str,
        mana: &'static str,
        types: &'static [&'static str],
        body: (u32, u32),
        cost_kinds: &'static [&'static str],
        mana_cost: Option<&'static str>,
        output: ManaAmount,
    }
    for case in [
        Case {
            id: "basal_thrull",
            mana_cost: None,
            name: "Basal Thrull",
            mana: "{B}{B}",
            types: &["Creature", "Thrull"],
            body: (1, 2),
            cost_kinds: &["tap", "sac"],
            output: ManaAmount {
                b: 2,
                ..Default::default()
            },
        },
        Case {
            id: "coal_golem",
            mana_cost: Some("{3}"),
            name: "Coal Golem",
            mana: "{5}",
            types: &["Artifact", "Creature", "Golem"],
            body: (3, 3),
            cost_kinds: &["mana", "sac"],
            output: ManaAmount {
                r: 3,
                ..Default::default()
            },
        },
        Case {
            id: "composite_golem",
            mana_cost: None,
            name: "Composite Golem",
            mana: "{6}",
            types: &["Artifact", "Creature", "Golem"],
            body: (4, 4),
            cost_kinds: &["sac"],
            output: ManaAmount {
                w: 1,
                u: 1,
                b: 1,
                r: 1,
                g: 1,
                ..Default::default()
            },
        },
        Case {
            id: "morgue_toad",
            mana_cost: None,
            name: "Morgue Toad",
            mana: "{2}{B}",
            types: &["Creature", "Frog"],
            body: (2, 2),
            cost_kinds: &["sac"],
            output: ManaAmount {
                u: 1,
                r: 1,
                ..Default::default()
            },
        },
        Case {
            id: "overeager_apprentice",
            mana_cost: None,
            name: "Overeager Apprentice",
            mana: "{2}{B}",
            types: &["Creature", "Human", "Minion"],
            body: (1, 2),
            cost_kinds: &["discard", "sac"],
            output: ManaAmount {
                b: 3,
                ..Default::default()
            },
        },
        Case {
            id: "satyr_hedonist",
            mana_cost: Some("{R}"),
            name: "Satyr Hedonist",
            mana: "{1}{G}",
            types: &["Creature", "Satyr"],
            body: (2, 1),
            cost_kinds: &["mana", "sac"],
            output: ManaAmount {
                r: 3,
                ..Default::default()
            },
        },
    ] {
        let f = identity(case.id, case.name, case.mana, case.types, Some(case.body));
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: one mana ability", case.id);
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{}",
            case.id
        );
        assert_eq!(ability.costs.len(), case.cost_kinds.len(), "{}", case.id);
        for (cost, kind) in ability.costs.iter().zip(case.cost_kinds) {
            match *kind {
                "tap" => assert!(matches!(cost, AbilityCost::Tap), "{}", case.id),
                "sac" => assert!(matches!(cost, AbilityCost::SacrificeSelf), "{}", case.id),
                "discard" => assert!(matches!(cost, AbilityCost::Discard), "{}", case.id),
                "mana" => {
                    let AbilityCost::Mana(mana_cost) = cost else {
                        panic!("{}: mana cost", case.id);
                    };
                    assert_eq!(
                        mana_cost.to_string(),
                        case.mana_cost.expect("printed mana cost"),
                        "{}",
                        case.id
                    );
                }
                other => panic!("{}: unknown cost kind {other}", case.id),
            }
        }
        let [SpellEffectKind::ProduceMana { options, .. }] = ability.effect.as_slice() else {
            panic!("{}: ProduceMana", case.id);
        };
        assert_eq!(options, &[case.output], "{}", case.id);
    }
}

// ── 08 Raid simple ─────────────────────────────────────────────────────────

#[test]
fn q49_raid_maps_definitions() {
    for (id, name, mana, types, body) in [
        (
            "deadeye_tormentor",
            "Deadeye Tormentor",
            "{2}{B}",
            &["Creature", "Human", "Pirate"][..],
            (2, 2),
        ),
        (
            "mardu_heart-piercer",
            "Mardu Heart-Piercer",
            "{3}{R}",
            &["Creature", "Human", "Archer"][..],
            (2, 3),
        ),
        (
            "storm_fleet_pyromancer",
            "Storm Fleet Pyromancer",
            "{4}{R}",
            &["Creature", "Human", "Pirate", "Wizard"][..],
            (3, 2),
        ),
        (
            "swaggering_corsair",
            "Swaggering Corsair",
            "{2}{R}",
            &["Creature", "Human", "Pirate"][..],
            (2, 2),
        ),
    ] {
        let f = identity(id, name, mana, types, Some(body));
        match id {
            "swaggering_corsair" => {
                let [static_ability] = f.static_abilities.as_slice() else {
                    panic!("{id}: enters-with-counters static");
                };
                assert_eq!(
                    static_ability.presentation,
                    AbilityPresentation::OracleLines(vec![1]),
                    "{id}"
                );
                let StaticAbilityDef::EntersWithCounters {
                    counter, amount, ..
                } = &static_ability.definition
                else {
                    panic!("{id}: EntersWithCounters");
                };
                assert_eq!(*counter, CounterKind::PlusOnePlusOne, "{id}");
                let Amount::Conditional {
                    condition,
                    when_true,
                    otherwise,
                } = amount
                else {
                    panic!("{id}: conditional amount");
                };
                assert_eq!(
                    condition,
                    &GameCondition::AttackedThisTurn {
                        players: RelativePlayerSet::Controller
                    },
                    "{id}"
                );
                assert_eq!((*when_true, *otherwise), (1, 0), "{id}");
            }
            _ => {
                let [trigger] = f.triggered_abilities.as_slice() else {
                    panic!("{id}: one raid trigger");
                };
                assert_eq!(
                    trigger.intervening_if,
                    Some(GameCondition::AttackedThisTurn {
                        players: RelativePlayerSet::Controller
                    }),
                    "{id}"
                );
                assert_eq!(
                    trigger.presentation,
                    AbilityPresentation::OracleLines(vec![1]),
                    "{id}"
                );
                assert_eq!(
                    trigger.trigger,
                    TriggerCondition::WhenSelfEntersBattlefield,
                    "{id}"
                );
                match trigger.effect.as_slice() {
                    [SpellEffectKind::DamageTarget { amount, .. }] => {
                        assert_eq!(amount, &Amount::Fixed(2), "{id}");
                    }
                    [SpellEffectKind::ChooseHandCards {
                        action,
                        count,
                        target,
                        ..
                    }] => {
                        assert_eq!(*action, HandCardAction::Discard, "{id}");
                        assert_eq!(*count, 1, "{id}");
                        assert_eq!(
                            target.kind,
                            tricerules_cards::primitives::TargetKind::OpponentPlayer,
                            "{id}"
                        );
                    }
                    other => panic!("{id}: unexpected effect {other:?}"),
                }
            }
        }
        // Every raid identity carries the keyword-free printed body; the keyword line is
        // covered by the map and the actual-card test.
        assert!(f.keywords.is_empty(), "{id}: no printed keyword");
    }
}

#[test]
fn q49_batch_c_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, digest) in [
        (
            "archaeomender",
            "Archaeomender",
            "3b9f648a606ea5cfacaa146ad5243808e78161aa841a34561413418e80e73503",
        ),
        (
            "barrow_witches",
            "Barrow Witches",
            "af9c40ac1fc08fe283361cc00a52d5a1c346664475f8b5ed302397c05bbb2e0d",
        ),
        (
            "golgari_findbroker",
            "Golgari Findbroker",
            "db773a30cae08ee199c0c438a729d51beebbe713e18d00a74f25cd4a551bf311",
        ),
        (
            "monk_idealist",
            "Monk Idealist",
            "4bd571ead436e0e332a040103b5fad8c4406e8ebd5501e8a4d2fec9f97bbfed1",
        ),
        (
            "restoration_gearsmith",
            "Restoration Gearsmith",
            "68fa4658b3e2d0407c791a45e5dafb19f0ce60bce0e90be3265f5a2cacbfcf5a",
        ),
        (
            "warden_of_the_eye",
            "Warden of the Eye",
            "22a0665c3e18e23826e4afeb7aac73ca51f16c11d5487fba138a1cb8e53ed478",
        ),
        (
            "basal_thrull",
            "Basal Thrull",
            "42ff264872e8ab5ffbfdc3990efdb7b7ea0290252c071c56302c57a9eb7ddb6a",
        ),
        (
            "coal_golem",
            "Coal Golem",
            "48c2376c0438f9116d6de8e26e020f3d3304c2ee908f5f823f153051af6d803e",
        ),
        (
            "composite_golem",
            "Composite Golem",
            "4a18a1fc265a7f76863e0066a9816ca3a5d2ef64ffb6c6d5a43835dd25f5e180",
        ),
        (
            "morgue_toad",
            "Morgue Toad",
            "d3a8cbc0cec23de390d7f55df3d6aa3c7b8eaa20f4fc1ab3784afa3561b730e0",
        ),
        (
            "overeager_apprentice",
            "Overeager Apprentice",
            "b3cef1ac69f9b96312f10e8991abdad31ead03fa0281330102f36bb1a37fcb04",
        ),
        (
            "satyr_hedonist",
            "Satyr Hedonist",
            "375e6ea49a9ae6eac296e700e5daf9b44fa7ccde7457a28ba876699d39aa398f",
        ),
        (
            "deadeye_tormentor",
            "Deadeye Tormentor",
            "17db9a09bd5c3f67d5d0670a12ce407be7bac4ea620a76c5002230ed29a7bbd0",
        ),
        (
            "mardu_heart-piercer",
            "Mardu Heart-Piercer",
            "717d9a7f7f57d15ca55df993c25e438340142c42663829b6ae86859630a6aa81",
        ),
        (
            "storm_fleet_pyromancer",
            "Storm Fleet Pyromancer",
            "717d9a7f7f57d15ca55df993c25e438340142c42663829b6ae86859630a6aa81",
        ),
        (
            "swaggering_corsair",
            "Swaggering Corsair",
            "975e418623938db7e91252b1ceab7977c0419f847c8e728d870c6697ae382ca1",
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
        assert_eq!(fields[2], face_id(name), "face id drift for {id}");
        assert_eq!(fields[3], name);
        assert_eq!(fields[4], digest, "pinned fingerprint digest for {id}");
        assert_eq!(definition(id).primary_face().name, name, "{id}");
    }
}
