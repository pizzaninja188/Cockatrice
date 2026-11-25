//! Registry conformance for the queue-v81 01-etb-target-pump cohort.
//!
//! Every card is a normal creature whose complete rules text is one printed keyword plus one
//! mandatory entry trigger targeting any creature for a fixed until-end-of-turn pump. Fingerprints
//! are checked after the presentation metadata refresh.

use tricerules_cards::primitives::{EffectSubject, SpellEffectKind, TargetKind};
use tricerules_cards::{AbilityPresentation, Keyword, Layout, TriggerCondition};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    keywords: &'static [Keyword],
    pump_power: i32,
    pump_toughness: i32,
    oracle_line: u16,
    fingerprint: &'static str,
}

fn cases() -> Vec<Card> {
    vec![
        Card {
            id: "bone_pit_brute",
            name: "Bone Pit Brute",
            mana_cost: "{4}{R}{R}",
            types: &["Creature", "Cyclops"],
            power: 4,
            toughness: 5,
            keywords: &[Keyword::Menace],
            pump_power: 4,
            pump_toughness: 0,
            oracle_line: 2,
            fingerprint: "550a471ffb877853e815e9f9c5877331483a1dbf624e39152322b6491b890957",
        },
        Card {
            id: "daybreak_combatants",
            name: "Daybreak Combatants",
            mana_cost: "{2}{R}",
            types: &["Creature", "Human", "Warrior"],
            power: 2,
            toughness: 2,
            keywords: &[Keyword::Haste],
            pump_power: 2,
            pump_toughness: 0,
            oracle_line: 2,
            fingerprint: "c3364afb63bc74ee5717b4fd82bfd327587b00cb96533eb65c92b85f4da04d94",
        },
        Card {
            id: "soldier_of_the_grey_host",
            name: "Soldier of the Grey Host",
            mana_cost: "{3}{W}",
            types: &["Creature", "Spirit", "Soldier"],
            power: 2,
            toughness: 2,
            keywords: &[Keyword::Flash, Keyword::Flying],
            pump_power: 2,
            pump_toughness: 0,
            oracle_line: 3,
            fingerprint: "be2b8d96c3dbbee56c7744a1382f35456af6d71943d488dc4f95298fdfe89660",
        },
    ]
}

#[test]
fn q81_etb_pump_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in cases() {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(
            registry.id_for_name(card.name),
            Some(card.id),
            "{}",
            card.id
        );
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), card.id, "{} face id", card.id);
        assert_eq!(face.name, card.name, "{} name", card.id);
        assert_eq!(
            face.mana_cost.to_string(),
            card.mana_cost,
            "{} mana",
            card.id
        );
        assert_eq!(face.types, card.types, "{} types", card.id);
        assert_eq!(face.power, Some(card.power), "{} power", card.id);
        assert_eq!(
            face.toughness,
            Some(card.toughness),
            "{} toughness",
            card.id
        );
        assert_eq!(face.keywords, card.keywords, "{} keywords", card.id);
        assert!(face.protections.is_empty(), "{}", card.id);
        assert!(face.static_abilities.is_empty(), "{}", card.id);
        assert!(face.activated_abilities.is_empty(), "{}", card.id);
        assert!(face.spell_effect.is_empty(), "{}", card.id);

        let [trigger] = face.triggered_abilities.as_slice() else {
            panic!("{}: exactly one triggered ability", card.id);
        };
        assert_eq!(trigger.ability_id.as_str(), "triggered_01", "{}", card.id);
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![card.oracle_line]),
            "{}",
            card.id
        );
        assert_eq!(
            trigger.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{}",
            card.id
        );
        assert!(trigger.targeting.is_none(), "{}", card.id);
        let [SpellEffectKind::PumpTarget {
            power,
            toughness,
            subject,
            scale,
        }] = trigger.effect.as_slice()
        else {
            panic!("{}: one PumpTarget", card.id);
        };
        assert_eq!(*power, card.pump_power, "{} pump power", card.id);
        assert_eq!(
            *toughness, card.pump_toughness,
            "{} pump toughness",
            card.id
        );
        assert!(scale.is_none(), "{}", card.id);
        let EffectSubject::Chosen(filter) = subject else {
            panic!("{}: chosen creature subject", card.id);
        };
        assert_eq!(filter.kind, TargetKind::Creature, "{}", card.id);
        assert!(filter.permanent_types.is_empty(), "{}", card.id);
    }
}

#[test]
fn q81_etb_pump_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in cases() {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(fields[2], card.id, "face id drift for {}", card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
