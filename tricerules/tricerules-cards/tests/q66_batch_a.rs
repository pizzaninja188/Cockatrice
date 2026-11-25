//! Registry conformance for the queue-v66 batch A cohorts.
//!
//! Three stable typed shapes: an activated card-draw ability with printed mana and life costs,
//! an Aura that controls the enchanted creature, and a permanent that investigates by creating
//! one Clue token. Fingerprints are asserted once the presentation metadata is refreshed from the
//! pinned Oracle text.

use tricerules_cards::primitives::{AbilityCost, Amount, SpellEffectKind};
use tricerules_cards::{registry, AbilityPresentation, ActivatedAbilityDef, CardFace, Layout};

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

fn only_activated<'a>(f: &'a CardFace, id: &str) -> &'a ActivatedAbilityDef {
    let [ability] = f.activated_abilities.as_slice() else {
        panic!("{id}: one activated ability");
    };
    ability
}

fn draws_one(id: &str, ability: &ActivatedAbilityDef) {
    assert!(
        matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::Draw {
                count: Amount::Fixed(1),
                ..
            }]
        ),
        "{id}: {:?}",
        ability.effect
    );
}

#[test]
fn q66_activated_draw_map_definitions() {
    // Arcane Encyclopedia: {3}, {T}: Draw a card.
    let f = face(
        "arcane_encyclopedia",
        "Arcane Encyclopedia",
        "{3}",
        &["Artifact", "Book"],
    );
    let ability = only_activated(f, "arcane_encyclopedia");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(matches!(
        ability.costs.as_slice(),
        [AbilityCost::Mana(cost), AbilityCost::Tap] if cost.to_string() == "{3}"
    ));
    draws_one("arcane_encyclopedia", ability);

    // Azure Mage: {3}{U}: Draw a card, without tapping.
    let f = face(
        "azure_mage",
        "Azure Mage",
        "{1}{U}",
        &["Creature", "Human", "Wizard"],
    );
    assert_eq!((f.power, f.toughness), (Some(2), Some(1)));
    let ability = only_activated(f, "azure_mage");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(matches!(
        ability.costs.as_slice(),
        [AbilityCost::Mana(cost)] if cost.to_string() == "{3}{U}"
    ));
    draws_one("azure_mage", ability);

    // Book of Rass: {2}, Pay 2 life: Draw a card.
    let f = face("book_of_rass", "Book of Rass", "{6}", &["Artifact", "Book"]);
    let ability = only_activated(f, "book_of_rass");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(matches!(
        ability.costs.as_slice(),
        [AbilityCost::Mana(cost), AbilityCost::PayLife { amount: 2 }]
            if cost.to_string() == "{2}"
    ));
    draws_one("book_of_rass", ability);

    // Greed: {B}, Pay 2 life: Draw a card.
    let f = face("greed", "Greed", "{3}{B}", &["Enchantment"]);
    let ability = only_activated(f, "greed");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(matches!(
        ability.costs.as_slice(),
        [AbilityCost::Mana(cost), AbilityCost::PayLife { amount: 2 }]
            if cost.to_string() == "{B}"
    ));
    draws_one("greed", ability);
}

#[test]
fn q66_control_aura_map_definitions() {
    use tricerules_cards::primitives::{
        Keyword, ObjectFilter, SpellEffectKind, StaticAbilityDef, TargetKind,
    };

    for (id, name, mana) in [
        ("control_magic", "Control Magic", "{2}{U}{U}"),
        ("persuasion", "Persuasion", "{3}{U}{U}"),
    ] {
        let f = face(id, name, mana, &["Enchantment", "Aura"]);
        assert_eq!(
            f.spell_effect,
            [SpellEffectKind::AuraAttach {
                target: ObjectFilter {
                    kind: TargetKind::Creature,
                    ..ObjectFilter::default()
                },
            }],
            "{id}"
        );
        assert_eq!(
            f.static_abilities,
            [StaticAbilityDef::ControlsAttached],
            "{id}"
        );
    }

    let f = face(
        "spirit_away",
        "Spirit Away",
        "{5}{U}{U}",
        &["Enchantment", "Aura"],
    );
    assert_eq!(
        f.spell_effect,
        [SpellEffectKind::AuraAttach {
            target: ObjectFilter {
                kind: TargetKind::Creature,
                ..ObjectFilter::default()
            },
        }]
    );
    assert_eq!(f.static_abilities.len(), 2, "control plus attached pump");
    assert_eq!(f.static_abilities[0], StaticAbilityDef::ControlsAttached);
    assert!(
        matches!(
            &f.static_abilities[1].definition,
            StaticAbilityDef::AttachedModifier {
                delta_power: 2,
                delta_toughness: 2,
                keywords,
                ..
            } if keywords.as_slice() == [Keyword::Flying]
        ),
        "{:?}",
        f.static_abilities[1].definition
    );
}

#[test]
fn q66_investigate_permanent_map_definitions() {
    use tricerules_cards::primitives::{SpellEffectKind, TriggerCondition};

    for (id, name, mana, types, power, toughness) in [
        (
            "thraben_inspector",
            "Thraben Inspector",
            "{W}",
            &["Creature", "Human", "Soldier"][..],
            1,
            2,
        ),
        (
            "drownyard_explorers",
            "Drownyard Explorers",
            "{3}{U}",
            &["Creature", "Human", "Wizard"][..],
            2,
            4,
        ),
    ] {
        let f = face(id, name, mana, types);
        assert_eq!(
            (f.power, f.toughness),
            (Some(power), Some(toughness)),
            "{id}"
        );
        let [ability] = f.triggered_abilities.as_slice() else {
            panic!("{id}: one entry trigger");
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::CreateTokens { token, count: Amount::Fixed(1), .. }]
                    if token == "clue"
            ),
            "{id}: {:?}",
            ability.effect
        );
    }

    let f = face(
        "floodhound",
        "Floodhound",
        "{U}",
        &["Creature", "Elemental", "Dog"],
    );
    assert_eq!((f.power, f.toughness), (Some(1), Some(2)));
    let ability = only_activated(f, "floodhound");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(matches!(
        ability.costs.as_slice(),
        [AbilityCost::Mana(cost), AbilityCost::Tap] if cost.to_string() == "{3}"
    ));
    assert!(matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::CreateTokens { token, count: Amount::Fixed(1), .. }]
            if token == "clue"
    ));

    let f = face("magnifying_glass", "Magnifying Glass", "{3}", &["Artifact"]);
    assert_eq!(
        f.activated_abilities.len(),
        2,
        "mana ability plus investigate"
    );
    let mana = &f.activated_abilities[0];
    assert_eq!(mana.presentation, AbilityPresentation::OracleLines(vec![1]));
    assert_eq!(mana.costs.as_slice(), [AbilityCost::Tap]);
    assert!(
        matches!(
            mana.effect.as_slice(),
            [SpellEffectKind::ProduceMana { options, .. }] if options.len() == 1
                && options[0].c == 1
        ),
        "{:?}",
        mana.effect
    );
    let investigate = &f.activated_abilities[1];
    assert_eq!(
        investigate.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(matches!(
        investigate.costs.as_slice(),
        [AbilityCost::Mana(cost), AbilityCost::Tap] if cost.to_string() == "{4}"
    ));
    assert!(matches!(
        investigate.effect.as_slice(),
        [SpellEffectKind::Investigate {
            count: Amount::Fixed(1),
            ..
        }]
    ));
}

#[test]
fn q66_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, face_id, fingerprint) in [
        (
            "arcane_encyclopedia",
            "Arcane Encyclopedia",
            "arcane_encyclopedia",
            "bf1e67d0e4e1aeb614f7a32f6bab65acee354e896e952f286b365985c530c6a5",
        ),
        (
            "azure_mage",
            "Azure Mage",
            "azure_mage",
            "6891a9be4a1ab8812352098ba5430ce806b748f1c996892ee3f7cec24a045573",
        ),
        (
            "book_of_rass",
            "Book of Rass",
            "book_of_rass",
            "0343b98baf2d92ffacd0829d4ab6ab5cf7fec3c606d1ed295b7cb82414c75730",
        ),
        (
            "greed",
            "Greed",
            "greed",
            "d32ccf4325ba2d41bd6901f2fcf2174b408538c98d4c247378aefd9b4442cfec",
        ),
        (
            "control_magic",
            "Control Magic",
            "control_magic",
            "d486ad0002ddd59bd73e52f10a69857c777e4f049d3fbb47f9e5d6298fb3b49f",
        ),
        (
            "persuasion",
            "Persuasion",
            "persuasion",
            "9f491ef30986715581ab98897c0614aec4ff669b206e96be2d825e1b673a894a",
        ),
        (
            "spirit_away",
            "Spirit Away",
            "spirit_away",
            "b4264cc2a54c3f47653ad174f049fc1b003a3379cc021043d81df75979e29842",
        ),
        (
            "drownyard_explorers",
            "Drownyard Explorers",
            "drownyard_explorers",
            "a45b7ed1093d9993d47b43e0e95f31845c5b5caf529ac167dbcf82808826973d",
        ),
        (
            "thraben_inspector",
            "Thraben Inspector",
            "thraben_inspector",
            "a45b7ed1093d9993d47b43e0e95f31845c5b5caf529ac167dbcf82808826973d",
        ),
        (
            "floodhound",
            "Floodhound",
            "floodhound",
            "9af118b9e6bf32db75fa2a70d668aea91b613ca96b3177778e11cfc435f89407",
        ),
        (
            "magnifying_glass",
            "Magnifying Glass",
            "magnifying_glass",
            "298c88a62e044bdb53f5bec39282c5992afc67c6d87e2a1f2bcfed96875422ae",
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
        assert_eq!(fields[2], face_id);
        assert_eq!(fields[3], name, "display name drift for {id}");
        assert_eq!(fields[4], fingerprint, "fingerprint drift for {id}");
    }
}
