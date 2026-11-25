//! Registry conformance for the queue-v67 batch A cohorts.
//!
//! Three stable typed shapes: a printed keyword plus one activated self-pump, an Aura that keeps
//! the enchanted creature from untapping (with printed entry riders), and creatures whose only
//! keyword is Ward (with a printed cost-reduction rider on the hardest card). Fingerprints are
//! asserted once the presentation metadata is refreshed from the pinned Oracle text.

use tricerules_cards::primitives::{
    CastTriggerPlayer, EffectSubject, InstructionKind, ResolutionCost, SpellCostModifier,
    SpellEffectKind, StaticAbilityDef, TargetingSourceFilter, TriggerCondition,
};
use tricerules_cards::{registry, AbilityCost, AbilityPresentation, CardFace, Keyword, Layout};

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

fn only_activated<'a>(f: &'a CardFace, id: &str) -> &'a tricerules_cards::ActivatedAbilityDef {
    let [ability] = f.activated_abilities.as_slice() else {
        panic!("{id}: one activated ability");
    };
    ability
}

#[test]
fn q67_cohorts_maps_definitions() {
    // 01: keyword plus one activated self-pump, paid with the exact printed mana.
    for (id, name, mana, types, power, toughness, keyword, cost, dp, dt) in [
        (
            "anointed_chorister",
            "Anointed Chorister",
            "{W}",
            &["Creature", "Human", "Cleric"][..],
            1,
            1,
            Keyword::Lifelink,
            "{4}{W}",
            3,
            3,
        ),
        (
            "pardic_collaborator",
            "Pardic Collaborator",
            "{3}{R}",
            &["Creature", "Human", "Barbarian"][..],
            2,
            2,
            Keyword::FirstStrike,
            "{B}",
            1,
            1,
        ),
        (
            "rakdos_trumpeter",
            "Rakdos Trumpeter",
            "{1}{B}",
            &["Creature", "Human", "Shaman"][..],
            1,
            3,
            Keyword::Menace,
            "{3}{R}",
            2,
            0,
        ),
    ] {
        let f = face(id, name, mana, types);
        assert_eq!((f.power, f.toughness), (Some(power), Some(toughness)));
        assert_eq!(f.keywords, [keyword], "{id}");
        let ability = only_activated(f, id);
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert!(
            matches!(
                ability.costs.as_slice(),
                [AbilityCost::Mana(printed)] if printed.to_string() == cost
            ),
            "{id}: {:?}",
            ability.costs
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::PumpTarget {
                    power,
                    toughness,
                    subject: EffectSubject::Source,
                    ..
                }] if *power == dp && *toughness == dt
            ),
            "{id}: {:?}",
            ability.effect
        );
    }

    // 02: Aura untap lock, with the printed entry riders on the harder cards.
    for (id, name, mana, static_line, draws, taps, destroys) in [
        (
            "dehydration",
            "Dehydration",
            "{3}{U}",
            2,
            false,
            false,
            false,
        ),
        (
            "eternity_snare",
            "Eternity Snare",
            "{5}{U}",
            3,
            true,
            false,
            false,
        ),
        (
            "frozen_solid",
            "Frozen Solid",
            "{1}{U}{U}",
            2,
            false,
            false,
            true,
        ),
        (
            "mystic_restraints",
            "Mystic Restraints",
            "{2}{U}{U}",
            4,
            false,
            true,
            false,
        ),
    ] {
        let f = face(id, name, mana, &["Enchantment", "Aura"]);
        assert!(
            matches!(
                f.spell_effect.as_slice(),
                [SpellEffectKind::AuraAttach { .. }]
            ),
            "{id}: {:?}",
            f.spell_effect
        );
        assert_eq!(
            f.static_abilities.len(),
            1,
            "{id}: one attached static ability"
        );
        assert_eq!(
            f.static_abilities[0].presentation,
            AbilityPresentation::OracleLines(vec![static_line])
        );
        assert!(
            matches!(
                f.static_abilities[0].definition,
                StaticAbilityDef::AttachedModifier {
                    doesnt_untap_during_untap_step: true,
                    ..
                }
            ),
            "{id}: {:?}",
            f.static_abilities[0].definition
        );
        let triggers = &f.triggered_abilities;
        match (draws, taps, destroys) {
            (true, false, false) => {
                let [trigger] = triggers.as_slice() else {
                    panic!("{id}: one entry trigger");
                };
                assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
                assert_eq!(
                    trigger.presentation,
                    AbilityPresentation::OracleLines(vec![2])
                );
                assert!(matches!(
                    trigger.effect.as_slice(),
                    [SpellEffectKind::Draw {
                        count: tricerules_cards::Amount::Fixed(1),
                        ..
                    }]
                ));
            }
            (false, true, false) => {
                let [trigger] = triggers.as_slice() else {
                    panic!("{id}: one entry trigger");
                };
                assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
                assert_eq!(
                    trigger.presentation,
                    AbilityPresentation::OracleLines(vec![3])
                );
                assert!(matches!(
                    trigger.effect.as_slice(),
                    [SpellEffectKind::Tap { .. }]
                ));
            }
            (false, false, true) => {
                let [trigger] = triggers.as_slice() else {
                    panic!("{id}: one damage trigger");
                };
                assert_eq!(
                    trigger.trigger,
                    TriggerCondition::WheneverAttachedObjectIsDealtDamage
                );
                assert_eq!(
                    trigger.presentation,
                    AbilityPresentation::OracleLines(vec![3])
                );
                assert!(matches!(
                    trigger.effect.as_slice(),
                    [SpellEffectKind::Destroy { .. }]
                ));
            }
            (false, false, false) => assert!(triggers.is_empty(), "{id}"),
            _ => unreachable!(),
        }
        if id == "mystic_restraints" {
            assert_eq!(f.keywords, [Keyword::Flash]);
        }
    }

    // 03: Ward {N} Program leaf with exact payment cost; Purple Worm also reduces by exactly {2}.
    for (id, name, mana, types, power, toughness, cost) in [
        (
            "rimeshield_frost_giant",
            "Rimeshield Frost Giant",
            "{3}{U}{U}",
            &["Creature", "Giant", "Warrior"][..],
            4,
            5,
            "{3}",
        ),
        (
            "tomakul_honor_guard",
            "Tomakul Honor Guard",
            "{1}{G}",
            &["Creature", "Human", "Soldier"][..],
            3,
            1,
            "{2}",
        ),
        (
            "purple_worm",
            "Purple Worm",
            "{5}{G}{G}",
            &["Creature", "Worm"][..],
            8,
            7,
            "{2}",
        ),
    ] {
        let f = face(id, name, mana, types);
        assert_eq!((f.power, f.toughness), (Some(power), Some(toughness)));
        let [ward] = f.triggered_abilities.as_slice() else {
            panic!("{id}: one Ward trigger");
        };
        assert_eq!(
            ward.trigger,
            TriggerCondition::WheneverSelfBecomesTarget {
                source: TargetingSourceFilter::SpellOrAbility,
                source_controller: CastTriggerPlayer::Opponent,
            }
        );
        assert_eq!(
            ward.presentation,
            AbilityPresentation::OracleLines(vec![if id == "purple_worm" { 2 } else { 1 }])
        );
        assert!(ward.effect.is_empty(), "{id}: Ward is a Program");
        let program = ward.program.as_ref().expect("Ward Program");
        assert!(program.body.yields.is_empty());
        let [instruction] = program.body.steps.as_slice() else {
            panic!("{id}: one Ward leaf");
        };
        let InstructionKind::Perform(effect) = &instruction.kind else {
            panic!("{id}: Ward is a Perform leaf");
        };
        assert_eq!(
            **effect,
            SpellEffectKind::CounterTriggeringStackObjectUnlessPays {
                cost: ResolutionCost::Mana(tricerules_cards::ManaCost::parse(cost).unwrap()),
            }
        );
    }

    let worm = face(
        "purple_worm",
        "Purple Worm",
        "{5}{G}{G}",
        &["Creature", "Worm"],
    );
    assert!(
        matches!(
            worm.cost_modifiers.as_slice(),
            [SpellCostModifier::ConditionalGenericReduction {
                amount: 2,
                condition: tricerules_cards::primitives::GameCondition::CreatureDeathsThisTurn {
                    min: Some(1),
                    ..
                },
            }]
        ),
        "{:?}",
        worm.cost_modifiers
    );
}

#[test]
fn q67_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "anointed_chorister",
            "Anointed Chorister",
            "d5322c570906255477be7b7d8b485654920743d1523148872492914e7537341d",
        ),
        (
            "pardic_collaborator",
            "Pardic Collaborator",
            "2db5135fbb5ca932f289d03611858df7798d88b4a415a310759c9f2236c3dbd5",
        ),
        (
            "rakdos_trumpeter",
            "Rakdos Trumpeter",
            "f6c3527e49034a8dc610b604e060c12c1db08c9ba89a63d9625466c4488ff8f9",
        ),
        (
            "dehydration",
            "Dehydration",
            "cdc37b7601acaef066ec71247e3aa8e9b7248642b8102da371b30d291e9ae900",
        ),
        (
            "eternity_snare",
            "Eternity Snare",
            "8dddafe4014de7d5afde9a28324b360524489c566715b7bae292eafde9bd4941",
        ),
        (
            "frozen_solid",
            "Frozen Solid",
            "6fb5a4d90e00c5faf264a0f4eb65b4b8ee6791d7044e2164d9f0aa5c91411f28",
        ),
        (
            "mystic_restraints",
            "Mystic Restraints",
            "1dbe320a7758d570a86c5a5acd30e80b4029173344479c1932e7e9172bb4c730",
        ),
        (
            "rimeshield_frost_giant",
            "Rimeshield Frost Giant",
            "ea1572803dd7abcc1b02777c00f47cc6a27dc333955101830e6db99f76faf251",
        ),
        (
            "tomakul_honor_guard",
            "Tomakul Honor Guard",
            "ac5cc54ef9ee4f417a441742ed672fac5d7519d2bfbea61142e026bce5973367",
        ),
        (
            "purple_worm",
            "Purple Worm",
            "6b6b0fc6599941ed41074bb56caffc60a5b525a694dc351bb45cee073164e1f5",
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
        assert_eq!(fields[2], id, "face id drift for {id}");
        assert_eq!(fields[3], name, "display name drift for {id}");
        assert_eq!(fields[4], fingerprint, "pinned Oracle fingerprint for {id}");
    }
}
