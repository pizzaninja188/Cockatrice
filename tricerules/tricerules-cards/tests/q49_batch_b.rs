//! Registry conformance for the queue-v49 reviewed cohorts (batch B).
#![allow(clippy::type_complexity)]
use tricerules_cards::primitives::{
    Amount, CastTriggerPlayer, CreatureScopeController, EffectSubject, Keyword, LifeAmount,
    PermanentTypeFilter, PlayerRecipient, SpellEffectKind, StaticAbilityDef, TargetKind,
    TriggerCondition,
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

// ── 04 Flash Aura pump plus keyword ────────────────────────────────────────

#[test]
fn q49_flash_aura_maps_definitions() {
    struct Case {
        id: &'static str,
        name: &'static str,
        mana: &'static str,
        delta: (i32, i32),
        keyword: Keyword,
    }
    for case in [
        Case {
            id: "epic_proportions",
            name: "Epic Proportions",
            mana: "{4}{G}{G}",
            delta: (5, 5),
            keyword: Keyword::Trample,
        },
        Case {
            id: "flaming_sword",
            name: "Flaming Sword",
            mana: "{1}{R}",
            delta: (1, 0),
            keyword: Keyword::FirstStrike,
        },
        Case {
            id: "spiteful_motives",
            name: "Spiteful Motives",
            mana: "{3}{R}",
            delta: (3, 0),
            keyword: Keyword::FirstStrike,
        },
        Case {
            id: "tiger_claws",
            name: "Tiger Claws",
            mana: "{2}{G}",
            delta: (1, 1),
            keyword: Keyword::Trample,
        },
    ] {
        let f = identity(
            case.id,
            case.name,
            case.mana,
            &["Enchantment", "Aura"],
            None,
        );
        assert_eq!(f.keywords, [Keyword::Flash], "{}", case.id);
        match f.spell_effect.as_slice() {
            [SpellEffectKind::AuraAttach { target }] => {
                assert_eq!(target.kind, TargetKind::Creature, "{}", case.id);
            }
            other => panic!("{}: expected AuraAttach, got {other:?}", case.id),
        }
        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{}: one static ability", case.id);
        };
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![3]),
            "{}",
            case.id
        );
        let StaticAbilityDef::AttachedModifier {
            delta_power,
            delta_toughness,
            keywords,
            ..
        } = &static_ability.definition
        else {
            panic!("{}: AttachedModifier", case.id);
        };
        assert_eq!((*delta_power, *delta_toughness), case.delta, "{}", case.id);
        assert_eq!(keywords, &[case.keyword], "{}", case.id);
    }
}

// ── 05 Landfall simple ─────────────────────────────────────────────────────

#[test]
fn q49_landfall_maps_definitions() {
    for (id, name, mana, types, body) in [
        (
            "baloth_woodcrasher",
            "Baloth Woodcrasher",
            "{4}{G}{G}",
            &["Creature", "Beast"][..],
            (4, 4),
        ),
        (
            "canyon_jerboa",
            "Canyon Jerboa",
            "{2}{W}",
            &["Creature", "Mouse"][..],
            (1, 2),
        ),
        (
            "kazandu_nectarpot",
            "Kazandu Nectarpot",
            "{1}{G}",
            &["Creature", "Insect"][..],
            (1, 3),
        ),
        (
            "tunneling_geopede",
            "Tunneling Geopede",
            "{2}{R}",
            &["Creature", "Insect"][..],
            (3, 2),
        ),
    ] {
        let f = identity(id, name, mana, types, Some(body));
        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{id}: one landfall trigger");
        };
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        let TriggerCondition::WheneverPermanentEntersBattlefield {
            controller, filter, ..
        } = &trigger.trigger
        else {
            panic!("{id}: landfall trigger");
        };
        assert_eq!(*controller, CastTriggerPlayer::Controller, "{id}");
        assert_eq!(
            filter.permanent_type,
            Some(PermanentTypeFilter::Land),
            "{id}"
        );
        match id {
            "baloth_woodcrasher" => {
                let [pump, grant] = trigger.effect.as_slice() else {
                    panic!("{id}: pump then grant");
                };
                let SpellEffectKind::PumpTarget {
                    power,
                    toughness,
                    subject,
                    ..
                } = pump
                else {
                    panic!("{id}: PumpTarget");
                };
                assert_eq!((*power, *toughness), (4, 4), "{id}");
                assert_eq!(subject, &EffectSubject::Source, "{id}");
                let SpellEffectKind::GrantKeywords { subject, keywords } = grant else {
                    panic!("{id}: GrantKeywords");
                };
                assert_eq!(subject, &EffectSubject::Source, "{id}");
                assert_eq!(keywords, &[Keyword::Trample], "{id}");
            }
            "canyon_jerboa" => {
                let [SpellEffectKind::PumpAll {
                    filter,
                    power,
                    toughness,
                }] = trigger.effect.as_slice()
                else {
                    panic!("{id}: PumpAll");
                };
                assert_eq!((*power, *toughness), (1, 1), "{id}");
                assert_eq!(
                    filter.controller,
                    Some(CreatureScopeController::YouControl),
                    "{id}"
                );
            }
            "kazandu_nectarpot" => {
                let [SpellEffectKind::GainLife { amount, .. }] = trigger.effect.as_slice() else {
                    panic!("{id}: GainLife");
                };
                assert_eq!(amount, &Amount::Fixed(1), "{id}");
            }
            "tunneling_geopede" => {
                let [SpellEffectKind::DamagePlayer { amount, who }] = trigger.effect.as_slice()
                else {
                    panic!("{id}: DamagePlayer");
                };
                assert_eq!(amount, &Amount::Fixed(1), "{id}");
                assert_eq!(who, &PlayerRecipient::EachOpponent, "{id}");
            }
            other => panic!("unhandled {other}"),
        }
    }
}

// ── 09 Becomes tapped triggers ─────────────────────────────────────────────

#[test]
fn q49_becomes_tapped_maps_definitions() {
    for (id, name, mana, types, body, presentation) in [
        (
            "goblin_medics",
            "Goblin Medics",
            "{2}{R}",
            &["Creature", "Goblin", "Shaman"][..],
            (1, 1),
            1,
        ),
        (
            "moonfolk_puzzlemaker",
            "Moonfolk Puzzlemaker",
            "{2}{U}",
            &["Artifact", "Creature", "Moonfolk", "Wizard"][..],
            (1, 4),
            2,
        ),
        (
            "night_market_lookout",
            "Night Market Lookout",
            "{B}",
            &["Creature", "Human", "Rogue"][..],
            (1, 1),
            1,
        ),
        (
            "samite_herbalist",
            "Samite Herbalist",
            "{1}{W}",
            &["Creature", "Human", "Cleric"][..],
            (2, 1),
            1,
        ),
        (
            "spireside_infiltrator",
            "Spireside Infiltrator",
            "{2}{R}",
            &["Creature", "Human", "Rogue"][..],
            (3, 2),
            1,
        ),
    ] {
        let f = identity(id, name, mana, types, Some(body));
        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{id}: one becomes-tapped trigger");
        };
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![presentation]),
            "{id}"
        );
        assert_eq!(
            trigger.trigger,
            TriggerCondition::WheneverSelfBecomesTapped,
            "{id}"
        );
        match id {
            "goblin_medics" => {
                let [SpellEffectKind::DamageTarget { amount, .. }] = trigger.effect.as_slice()
                else {
                    panic!("{id}: DamageTarget");
                };
                assert_eq!(amount, &Amount::Fixed(1), "{id}");
            }
            "moonfolk_puzzlemaker" => {
                assert_eq!(f.keywords, [Keyword::Flying], "{id}");
                let [SpellEffectKind::Scry { count }] = trigger.effect.as_slice() else {
                    panic!("{id}: Scry");
                };
                assert_eq!(count, &Amount::Fixed(1), "{id}");
            }
            "night_market_lookout" => {
                let [lose, gain] = trigger.effect.as_slice() else {
                    panic!("{id}: lose then gain");
                };
                let SpellEffectKind::LoseLife { amount, who } = lose else {
                    panic!("{id}: LoseLife");
                };
                assert_eq!(amount, &LifeAmount::Fixed(1), "{id}");
                assert_eq!(who, &PlayerRecipient::EachOpponent, "{id}");
                let SpellEffectKind::GainLife { amount, .. } = gain else {
                    panic!("{id}: GainLife");
                };
                assert_eq!(amount, &Amount::Fixed(1), "{id}");
            }
            "samite_herbalist" => {
                let [gain, scry] = trigger.effect.as_slice() else {
                    panic!("{id}: gain then scry");
                };
                let SpellEffectKind::GainLife { amount, .. } = gain else {
                    panic!("{id}: GainLife");
                };
                assert_eq!(amount, &Amount::Fixed(1), "{id}");
                let SpellEffectKind::Scry { count } = scry else {
                    panic!("{id}: Scry");
                };
                assert_eq!(count, &Amount::Fixed(1), "{id}");
            }
            "spireside_infiltrator" => {
                let [SpellEffectKind::DamagePlayer { amount, who }] = trigger.effect.as_slice()
                else {
                    panic!("{id}: DamagePlayer");
                };
                assert_eq!(amount, &Amount::Fixed(1), "{id}");
                assert_eq!(who, &PlayerRecipient::EachOpponent, "{id}");
            }
            other => panic!("unhandled {other}"),
        }
    }
}

#[test]
fn q49_batch_b_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, digest) in [
        (
            "epic_proportions",
            "Epic Proportions",
            "612dacdaec9467dbc54cea474a7cc4cab038a72bc4d41a1513dcbe2161f8a901",
        ),
        (
            "flaming_sword",
            "Flaming Sword",
            "71a979168ea3e57a3284e5f876c1af50130af999f2d146fb8c9db787ca8c2bfa",
        ),
        (
            "spiteful_motives",
            "Spiteful Motives",
            "da93d3a8bd40126747baf64c08c157309466f18ba72e65b14c5a945685964e91",
        ),
        (
            "tiger_claws",
            "Tiger Claws",
            "3dfa7154f2fc3fd759b7f56de81a001153aa4074e9e3111f9bdd79b9efe40909",
        ),
        (
            "baloth_woodcrasher",
            "Baloth Woodcrasher",
            "2ec1c2ea3ee25eecc0b7660c4c1ec142f72800f6de6bff9ea4952063eaf0df93",
        ),
        (
            "canyon_jerboa",
            "Canyon Jerboa",
            "83749473bb0fd0d541d4be9d8560d2818433c4e86ed0f556b5ad516217d60958",
        ),
        (
            "kazandu_nectarpot",
            "Kazandu Nectarpot",
            "e282226628093e6a46d4c5424f9d9e21753e5178f3f839bff87288b07dfa0936",
        ),
        (
            "tunneling_geopede",
            "Tunneling Geopede",
            "ac4d6441eb2bdc3116812bc1050427fef35ac98d3b45dd827948ee6b78b8831c",
        ),
        (
            "goblin_medics",
            "Goblin Medics",
            "807fb028eacc3d7f7d47d64fcd5e56a775300d1ebf2827caf7e6916baf1b3489",
        ),
        (
            "moonfolk_puzzlemaker",
            "Moonfolk Puzzlemaker",
            "2fe899f0b0b94c7eae5c14f6de39cfb2adf029a32c368ac29793c85c09c25e8b",
        ),
        (
            "night_market_lookout",
            "Night Market Lookout",
            "10668dc655d59702224afa4e8f95b5c670160cb2b6cf31bfa7f0ff856d2c8fa9",
        ),
        (
            "samite_herbalist",
            "Samite Herbalist",
            "3d40f4e589ae4bc514396ecbb6711d8e13f1bb86e3dd080b92656e5c2d89e9e3",
        ),
        (
            "spireside_infiltrator",
            "Spireside Infiltrator",
            "d7be01f59567b9bc6f16b8819f25713bff89dcef0949861a0d45cabbbf3dfa87",
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
