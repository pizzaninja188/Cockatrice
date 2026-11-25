//! Registry conformance for the queue-v63 batch A cohorts.
//!
//! Three stable typed shapes: artifact destruction with one printed rider (life loss,
//! regeneration prevention, draw, scry or flashback), a conditional self-pump keyed to a
//! permanent type the controller has, and creature destruction followed by the destroyed
//! creature's controller losing life. Fingerprints are asserted once the presentation metadata is
//! refreshed from the pinned Oracle text.

use tricerules_cards::primitives::{
    AbilityCost, Amount, BattlefieldAggregate, CardTypeFilter, EffectSubject, GameCondition,
    Keyword, LifeAmount, PermanentTypeFilter, PlayerRecipient, RelativePlayerSet, SpellEffectKind,
    StaticAbilityDef, TargetFilter, TargetKind,
};
use tricerules_cards::{registry, CardFace, Color, Layout};

fn face(id: &str, name: &str, mana: &str, types: &[&str]) -> &'static CardFace {
    let definition = registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(registry::global().id_for_name(name), Some(id), "{id}");
    let f = definition.primary_face();
    assert_eq!(
        f.face_id.as_str(),
        id.replace([',', '-'], "_").replace("__", "_").as_str(),
        "{id} face id"
    );
    assert_eq!(f.name, name, "{id} name");
    assert_eq!(f.mana_cost.to_string(), mana, "{id} mana");
    assert_eq!(
        f.types.iter().map(String::as_str).collect::<Vec<_>>(),
        types,
        "{id} types"
    );
    f
}

fn chosen<'a>(subject: &'a EffectSubject, id: &str) -> &'a TargetFilter {
    let EffectSubject::Chosen(filter) = subject else {
        panic!("{id}: expected a chosen subject, got {subject:?}");
    };
    filter
}

fn one_target(f: &CardFace, id: &str) {
    let groups = &f.targeting.as_ref().expect("targeting").groups;
    assert_eq!(groups.len(), 1, "{id}");
    assert_eq!((groups[0].min, groups[0].max), (1, 1), "{id}");
    assert_eq!(groups[0].effect_indices, [0], "{id}");
}

#[test]
fn q63_artifact_riders_map_definitions() {
    for (id, name, mana, rider) in [
        ("glissas_scorn", "Glissa's Scorn", "{1}{G}", "life"),
        ("oxidize", "Oxidize", "{G}", "prevent"),
        ("smash", "Smash", "{2}{R}", "draw"),
        ("tel-jilad_justice", "Tel-Jilad Justice", "{1}{G}", "scry"),
        ("bash_to_bits", "Bash to Bits", "{3}{R}", "flashback"),
    ] {
        let f = face(id, name, mana, &["Instant"]);
        let subject = match rider {
            "prevent" => match &f.spell_effect[0] {
                SpellEffectKind::DestroyPreventingRegeneration { subject } => subject,
                other => panic!("{id}: wrong destruction primitive: {other:?}"),
            },
            _ => match &f.spell_effect[0] {
                SpellEffectKind::Destroy { subject } => subject,
                other => panic!("{id}: wrong destruction primitive: {other:?}"),
            },
        };
        let filter = chosen(subject, id);
        assert_eq!(filter.kind, TargetKind::AnyPermanent, "{id}");
        assert_eq!(
            filter.permanent_types,
            [PermanentTypeFilter::Artifact],
            "{id}: exactly artifacts"
        );
        one_target(f, id);
        assert!(f.triggered_abilities.is_empty(), "{id}");
        match rider {
            "life" => assert!(matches!(
                &f.spell_effect[1],
                SpellEffectKind::LoseLife {
                    amount: LifeAmount::Fixed(1),
                    who: PlayerRecipient::ControllerOfTargetGroup { group_index: 0 },
                }
            )),
            "prevent" => assert_eq!(f.spell_effect.len(), 1, "{id}"),
            "draw" => assert!(matches!(
                f.spell_effect[1],
                SpellEffectKind::Draw {
                    count: Amount::Fixed(1),
                    ..
                }
            )),
            "scry" => assert!(matches!(
                f.spell_effect[1],
                SpellEffectKind::Scry {
                    count: Amount::Fixed(2)
                }
            )),
            "flashback" => {
                assert_eq!(f.spell_effect.len(), 1, "{id}");
                assert_eq!(
                    f.flashback_cost.as_ref().map(ToString::to_string),
                    Some("{4}{R}{R}".to_owned()),
                    "{id}"
                );
            }
            _ => unreachable!(),
        }
        if rider != "flashback" && rider != "prevent" {
            assert_eq!(f.spell_effect.len(), 2, "{id}");
        }
    }
}

#[test]
fn q63_conditional_pump_map_definitions() {
    for (id, name, mana, types, power, toughness, delta_power, delta_toughness, keyword) in [
        (
            "crimson_muckwader",
            "Crimson Muckwader",
            "{1}{R}",
            ["Creature", "Lizard"].as_slice(),
            2,
            1,
            1,
            1,
            None,
        ),
        (
            "sedge_troll",
            "Sedge Troll",
            "{2}{R}",
            ["Creature", "Troll"].as_slice(),
            2,
            2,
            1,
            1,
            None,
        ),
        (
            "prized_elephant",
            "Prized Elephant",
            "{3}{W}",
            ["Creature", "Elephant"].as_slice(),
            3,
            3,
            1,
            1,
            None,
        ),
        (
            "dhund_operative",
            "Dhund Operative",
            "{1}{B}",
            ["Creature", "Human", "Rogue"].as_slice(),
            2,
            2,
            1,
            0,
            Some(Keyword::Deathtouch),
        ),
        (
            "inventors_apprentice",
            "Inventor's Apprentice",
            "{R}",
            ["Creature", "Human", "Artificer"].as_slice(),
            1,
            2,
            1,
            1,
            None,
        ),
    ] {
        let f = face(id, name, mana, types);
        assert_eq!(
            (f.power, f.toughness),
            (Some(power), Some(toughness)),
            "{id}"
        );
        let [static_ability] = f.static_abilities.as_slice() else {
            panic!(
                "{id}: one conditional self modifier: {:?}",
                f.static_abilities
            );
        };
        let StaticAbilityDef::ConditionalSelfModifier {
            condition:
                GameCondition::BattlefieldAggregate {
                    filter,
                    aggregate,
                    min,
                    ..
                },
            delta_power: actual_power,
            delta_toughness: actual_toughness,
            keywords,
            ..
        } = &static_ability.definition
        else {
            panic!(
                "{id}: one conditional self modifier: {:?}",
                static_ability.definition
            );
        };
        assert_eq!(*actual_power, delta_power, "{id}");
        assert_eq!(*actual_toughness, delta_toughness, "{id}");
        assert_eq!(*min, Some(1), "{id}");
        assert_eq!(filter.controllers, RelativePlayerSet::Controller, "{id}");
        assert_eq!(*aggregate, BattlefieldAggregate::Count, "{id}");
        match id {
            "crimson_muckwader" | "sedge_troll" => {
                assert_eq!(filter.required_subtypes, ["Swamp"], "{id}")
            }
            "prized_elephant" => assert_eq!(filter.required_subtypes, ["Forest"], "{id}"),
            _ => assert_eq!(filter.card_type, Some(CardTypeFilter::Artifact), "{id}"),
        }
        match keyword {
            Some(keyword) => assert_eq!(keywords.as_slice(), [keyword], "{id}"),
            None => assert!(keywords.is_empty(), "{id}"),
        }
        match id {
            "crimson_muckwader" => {
                let [ability] = f.activated_abilities.as_slice() else {
                    panic!("{id}: one regenerate ability");
                };
                assert!(matches!(
                    ability.costs.as_slice(),
                    [AbilityCost::Mana(cost)] if cost.to_string() == "{2}{B}"
                ));
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::Regenerate {
                        subject: EffectSubject::Source
                    }]
                ));
            }
            "sedge_troll" => {
                let [ability] = f.activated_abilities.as_slice() else {
                    panic!("{id}: one regenerate ability");
                };
                assert!(matches!(
                    ability.costs.as_slice(),
                    [AbilityCost::Mana(cost)] if cost.to_string() == "{B}"
                ));
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::Regenerate {
                        subject: EffectSubject::Source
                    }]
                ));
            }
            "prized_elephant" => {
                let [ability] = f.activated_abilities.as_slice() else {
                    panic!("{id}: one trample ability");
                };
                assert!(matches!(
                    ability.costs.as_slice(),
                    [AbilityCost::Mana(cost)] if cost.to_string() == "{G}"
                ));
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::GrantKeywords {
                        keywords,
                        subject: EffectSubject::Source
                    }] if keywords.as_slice() == [Keyword::Trample]
                ));
            }
            _ => assert!(f.activated_abilities.is_empty(), "{id}"),
        }
        assert!(f.triggered_abilities.is_empty(), "{id}");
    }
}

#[test]
fn q63_destroy_drain_map_definitions() {
    for (id, name, mana, types, not_color, gain) in [
        (
            "certain_death",
            "Certain Death",
            "{5}{B}",
            ["Sorcery"].as_slice(),
            None,
            Some(2),
        ),
        (
            "sip_of_hemlock",
            "Sip of Hemlock",
            "{4}{B}{B}",
            ["Sorcery"].as_slice(),
            None,
            None,
        ),
        (
            "hideous_end",
            "Hideous End",
            "{1}{B}{B}",
            ["Instant"].as_slice(),
            Some(Color::Black),
            None,
        ),
    ] {
        let f = face(id, name, mana, types);
        let [SpellEffectKind::Destroy { subject }, drain @ ..] = f.spell_effect.as_slice() else {
            panic!("{id}: destroy then drain: {:?}", f.spell_effect);
        };
        let filter = chosen(subject, id);
        assert_eq!(filter.kind, TargetKind::Creature, "{id}");
        assert_eq!(filter.not_color, not_color, "{id}");
        assert_eq!(drain.len(), if gain.is_some() { 2 } else { 1 }, "{id}");
        assert!(matches!(
            drain[0],
            SpellEffectKind::LoseLife {
                amount: LifeAmount::Fixed(2),
                who: PlayerRecipient::ControllerOfTargetGroup { group_index: 0 },
            }
        ));
        if gain.is_some() {
            assert!(matches!(
                drain[1],
                SpellEffectKind::GainLife {
                    amount: Amount::Fixed(2),
                    ..
                }
            ));
        }
        one_target(f, id);
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
    }
}

#[test]
fn q63_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, face_id, fingerprint) in [
        (
            "bash_to_bits",
            "Bash to Bits",
            "bash_to_bits",
            "6f5f63993fbd9c1f95de7c49e436324c7c59139548cb0a3fdabb9911df3fb406",
        ),
        (
            "certain_death",
            "Certain Death",
            "certain_death",
            "0f986b63ff1184ea464f8ddbee5f6080bf384593eab15dee0760675b89629494",
        ),
        (
            "crimson_muckwader",
            "Crimson Muckwader",
            "crimson_muckwader",
            "508cfdd783e105235a5d634978eacfb7b474797fc317f6234a25631c2598c813",
        ),
        (
            "dhund_operative",
            "Dhund Operative",
            "dhund_operative",
            "f8fc8c949696bd9c3ccd47b9ac03611a2548be73af732ae9f1f15491a3b6c921",
        ),
        (
            "glissas_scorn",
            "Glissa's Scorn",
            "glissas_scorn",
            "06f3f94ab2088ce267e9406a2d95904f791dc813e7d2b605da6fe1c544218bd0",
        ),
        (
            "hideous_end",
            "Hideous End",
            "hideous_end",
            "a24060f5fd9b16c4b5a28a082c075e22dd268afd6aba1a0b265b53d0724d0ec5",
        ),
        (
            "inventors_apprentice",
            "Inventor's Apprentice",
            "inventors_apprentice",
            "5957b79272ea309ea9763fd9a6641d26efa2926eac5fe625b04a54857a31db2d",
        ),
        (
            "oxidize",
            "Oxidize",
            "oxidize",
            "1923c0201fd863dc663e3ea5aab89e4aa3d2d15aace12d861e05538a3218bc54",
        ),
        (
            "prized_elephant",
            "Prized Elephant",
            "prized_elephant",
            "9197ff41f5ebe42a79f70ca112e4f41c701c76ff5b9195da55303845852c80ab",
        ),
        (
            "sedge_troll",
            "Sedge Troll",
            "sedge_troll",
            "8c0c990dcc230b1a58bd019643b21b1b388ae04202ac4abb8815ee75c1f75b37",
        ),
        (
            "sip_of_hemlock",
            "Sip of Hemlock",
            "sip_of_hemlock",
            "95281c8a419da81fc49f2102be361f2a0477bf23d0a2506b8f7e69998a7efb88",
        ),
        (
            "smash",
            "Smash",
            "smash",
            "2e91a7da40b0eb66cbb19d44aaec2c197a506fd96f343d8aac8fad1e01fa9224",
        ),
        (
            "tel-jilad_justice",
            "Tel-Jilad Justice",
            "tel_jilad_justice",
            "444f2e60b15a0f672589c083acca5baccdb29cd79b4a2c900667e0cf0a154cfe",
        ),
    ] {
        let row = format!("{id}\t{name}\t{face_id}\t{name}\t{fingerprint}");
        assert!(
            fingerprints.lines().any(|line| line == row),
            "{id}: missing pinned fingerprint row"
        );
        assert_eq!(registry::global().id_for_name(name), Some(id), "{id}");
    }
}
