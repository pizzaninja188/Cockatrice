//! Registry conformance for the queue-v26 entry pump-and-flying cohort.
//!
//! Every card is a normal-layout flying creature whose entry trigger gives one
//! chosen creature the printed pump and flying until end of turn. Each
//! fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{EffectSubject, SpellEffectKind, TargetFilter};
use tricerules_cards::{AbilityPresentation, Keyword, Layout, TriggerCondition};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    pump_power: i32,
    pump_toughness: i32,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "welkin_guide",
        name: "Welkin Guide",
        mana_cost: "{4}{W}",
        types: &["Creature", "Bird", "Cleric"],
        power: 2,
        toughness: 2,
        pump_power: 2,
        pump_toughness: 2,
        fingerprint: "4c78284472626094bbcfb026a83b8fffff4ccce6240907493546b2b98fbf97a3",
    },
    Card {
        id: "arborea_pegasus",
        name: "Arborea Pegasus",
        mana_cost: "{3}{W}",
        types: &["Creature", "Pegasus"],
        power: 2,
        toughness: 3,
        pump_power: 1,
        pump_toughness: 1,
        fingerprint: "e6b4a55036f09a98f49b8d3c6f1c7fe6cd738b7606c1866311632e093fe609ee",
    },
    Card {
        id: "battleflight_eagle",
        name: "Battleflight Eagle",
        mana_cost: "{4}{W}",
        types: &["Creature", "Bird"],
        power: 2,
        toughness: 2,
        pump_power: 2,
        pump_toughness: 2,
        fingerprint: "4c78284472626094bbcfb026a83b8fffff4ccce6240907493546b2b98fbf97a3",
    },
];

fn face<'a>(
    registry: &'a tricerules_cards::CardRegistry,
    id: &str,
) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn etb_pump_flying_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
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
        let f = face(registry, card.id);
        assert_eq!(f.face_id.as_str(), card.id, "{} face id", card.id);
        assert_eq!(f.name, card.name, "{} name", card.id);
        assert_eq!(
            f.mana_cost.to_string(),
            card.mana_cost,
            "{} mana cost",
            card.id
        );
        assert_eq!(f.types, card.types, "{} types", card.id);
        assert!(f.supertypes.is_empty(), "{}", card.id);
        assert_eq!(
            (f.power, f.toughness),
            (Some(card.power), Some(card.toughness)),
            "{} body",
            card.id
        );
        assert_eq!(f.keywords, [Keyword::Flying], "{}", card.id);
        assert!(f.activated_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);
        assert!(f.targeting.is_none(), "{}", card.id);

        let [triggered] = f.triggered_abilities.as_slice() else {
            panic!("{}: exactly one entry trigger", card.id);
        };
        assert_eq!(triggered.ability_id.as_str(), "triggered_01", "{}", card.id);
        assert_eq!(
            triggered.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{}",
            card.id
        );
        assert_eq!(
            triggered.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{}",
            card.id
        );
        assert!(!triggered.may, "{}: the target is mandatory", card.id);

        let [SpellEffectKind::PumpTarget {
            power,
            toughness,
            subject: EffectSubject::Chosen(pump_filter),
            ..
        }, SpellEffectKind::GrantKeywords {
            subject: EffectSubject::Chosen(keyword_filter),
            keywords,
        }] = triggered.effect.as_slice()
        else {
            panic!("{}: pump then grant flying to one chosen creature", card.id);
        };
        assert_eq!(
            (*power, *toughness),
            (card.pump_power, card.pump_toughness),
            "{}",
            card.id
        );
        assert_eq!(
            pump_filter.as_ref(),
            &TargetFilter::default_creature(),
            "{} pump target",
            card.id
        );
        assert_eq!(
            keyword_filter.as_ref(),
            &TargetFilter::default_creature(),
            "{} keyword target",
            card.id
        );
        assert_eq!(keywords, &[Keyword::Flying], "{}", card.id);
    }
}

#[test]
fn etb_pump_flying_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(fields[2], card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
