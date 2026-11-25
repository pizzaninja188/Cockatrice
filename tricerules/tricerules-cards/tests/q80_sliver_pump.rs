//! Registry conformance for the queue-v80 01-sliver-pump-grant cohort.
//!
//! Every card grants one quoted `{2}` pump ability to all Sliver creatures. Fingerprints are
//! checked after the presentation metadata refresh.

use tricerules_cards::primitives::{
    AbilityCost, EffectSubject, SpellEffectKind, StaticAbilityDef, TargetKind,
};
use tricerules_cards::Layout;

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

fn cases() -> Vec<Card> {
    vec![
        Card {
            id: "armor_sliver",
            name: "Armor Sliver",
            mana_cost: "{2}{W}",
            types: &["Creature", "Sliver"],
            power: 2,
            toughness: 2,
            pump_power: 0,
            pump_toughness: 1,
            fingerprint: "32fe54de92a4606d76a6ef13d7238e1dda3820bed46c5891af8efe731b6d3dbe",
        },
        Card {
            id: "barbed_sliver",
            name: "Barbed Sliver",
            mana_cost: "{2}{R}",
            types: &["Creature", "Sliver"],
            power: 2,
            toughness: 2,
            pump_power: 1,
            pump_toughness: 0,
            fingerprint: "e1bac0bd1fb8d24f777641132cfb80218f8859533bdcc2bb6da084862079e789",
        },
        Card {
            id: "spectral_sliver",
            name: "Spectral Sliver",
            mana_cost: "{2}{B}",
            types: &["Creature", "Sliver", "Spirit"],
            power: 2,
            toughness: 2,
            pump_power: 1,
            pump_toughness: 1,
            fingerprint: "8ada6f7f01304454d66ddf508b71b6283faa27ce88df04cd760c09fbda514cf5",
        },
    ]
}

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
fn q80_sliver_pump_map_definitions() {
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
        let f = face(registry, card.id);
        assert_eq!(f.face_id.as_str(), card.id, "{} face id", card.id);
        assert_eq!(f.name, card.name, "{} name", card.id);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost, "{} mana", card.id);
        assert_eq!(f.types, card.types, "{} types", card.id);
        assert_eq!(f.power, Some(card.power), "{} power", card.id);
        assert_eq!(f.toughness, Some(card.toughness), "{} toughness", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.protections.is_empty(), "{}", card.id);
        assert!(f.activated_abilities.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);

        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{}: one static grant", card.id);
        };
        assert_eq!(
            static_ability.ability_id.as_str(),
            "static_01",
            "{}",
            card.id
        );
        let StaticAbilityDef::GrantActivatedAbilityToPermanents {
            filter,
            activated_abilities,
        } = &static_ability.definition
        else {
            panic!("{}: static grant shape", card.id);
        };
        assert_eq!(filter.kind, TargetKind::Creature, "{}", card.id);
        assert_eq!(
            filter.required_subtypes,
            vec!["Sliver".to_string()],
            "{}",
            card.id
        );
        assert!(filter.permanent_types.is_empty(), "{}", card.id);
        assert!(filter.excluded_subtypes.is_empty(), "{}", card.id);

        let [granted] = activated_abilities.as_slice() else {
            panic!("{}: one granted ability", card.id);
        };
        assert_eq!(granted.ability_id.as_str(), "activated_01", "{}", card.id);
        assert!(
            granted.source_zone == tricerules_cards::primitives::AbilitySourceZone::Battlefield,
            "{}",
            card.id
        );
        let [AbilityCost::Mana(mana)] = granted.costs.as_slice() else {
            panic!("{}: one mana cost", card.id);
        };
        assert_eq!(mana.to_string(), "{2}", "{}", card.id);
        let [SpellEffectKind::PumpTarget {
            power,
            toughness,
            subject,
            scale,
        }] = granted.effect.as_slice()
        else {
            panic!("{}: pump effect", card.id);
        };
        assert_eq!(*power, card.pump_power, "{} pump power", card.id);
        assert_eq!(
            *toughness, card.pump_toughness,
            "{} pump toughness",
            card.id
        );
        assert_eq!(*subject, EffectSubject::Source, "{}", card.id);
        assert!(scale.is_none(), "{}", card.id);
    }
}

#[test]
fn q80_sliver_pump_fingerprints_match_the_pinned_oracle_text() {
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
