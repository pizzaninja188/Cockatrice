//! Registry conformance for the reviewed becomes-target sacrifice cohort.
//!
//! Every included card is a normal-layout creature with one trigger on becoming
//! the target of a spell or ability controlled by any player, and an effect that
//! sacrifices the source. Each fingerprint is SHA-256 of the pinned Oracle lines.

use tricerules_cards::primitives::{
    CastTriggerPlayer, SpellEffectKind, TargetingSourceFilter, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "frost_walker",
        name: "Frost Walker",
        mana_cost: "{1}{U}",
        types: &["Creature", "Elemental"],
        power: 4,
        toughness: 1,
        fingerprint: "4f9704fe31e8d7548a970bc89eceb1360c3dece5b3b32a95c4e9adeddb47c375",
    },
    Card {
        id: "phantasmal_bear",
        name: "Phantasmal Bear",
        mana_cost: "{U}",
        types: &["Creature", "Bear", "Illusion"],
        power: 2,
        toughness: 2,
        fingerprint: "4f9704fe31e8d7548a970bc89eceb1360c3dece5b3b32a95c4e9adeddb47c375",
    },
    Card {
        id: "phantom_beast",
        name: "Phantom Beast",
        mana_cost: "{3}{U}",
        types: &["Creature", "Illusion", "Beast"],
        power: 4,
        toughness: 5,
        fingerprint: "4f9704fe31e8d7548a970bc89eceb1360c3dece5b3b32a95c4e9adeddb47c375",
    },
    Card {
        id: "skulking_fugitive",
        name: "Skulking Fugitive",
        mana_cost: "{2}{B}",
        types: &["Creature", "Horror", "Mercenary"],
        power: 3,
        toughness: 4,
        fingerprint: "4f9704fe31e8d7548a970bc89eceb1360c3dece5b3b32a95c4e9adeddb47c375",
    },
    Card {
        id: "tar_pit_warrior",
        name: "Tar Pit Warrior",
        mana_cost: "{2}{B}",
        types: &["Creature", "Cyclops", "Warrior"],
        power: 3,
        toughness: 4,
        fingerprint: "4f9704fe31e8d7548a970bc89eceb1360c3dece5b3b32a95c4e9adeddb47c375",
    },
];

fn face<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn becomes_target_sacrifice_batch_maps_definitions() {
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
        assert_eq!(f.power, Some(card.power), "{} power", card.id);
        assert_eq!(f.toughness, Some(card.toughness), "{} toughness", card.id);
        assert!(f.keywords.is_empty(), "{} no keyword", card.id);
        assert!(
            f.static_abilities.is_empty(),
            "{} no static ability",
            card.id
        );
        assert!(
            f.activated_abilities.is_empty(),
            "{} no activated ability",
            card.id
        );
        assert!(f.spell_effect.is_empty(), "{} no spell effect", card.id);

        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{}: exactly one triggered ability", card.id);
        };
        assert_eq!(trigger.ability_id.as_str(), "triggered_01", "{}", card.id);
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{} presentation",
            card.id
        );
        assert_eq!(
            trigger.trigger,
            TriggerCondition::WheneverSelfBecomesTarget {
                source: TargetingSourceFilter::SpellOrAbility,
                source_controller: CastTriggerPlayer::AnyPlayer,
            },
            "{}: spell or ability from any player",
            card.id
        );
        assert!(
            matches!(
                trigger.effect.as_slice(),
                [SpellEffectKind::Sacrifice { subject }]
                    if *subject == tricerules_cards::primitives::EffectSubject::Source
            ),
            "{}: sacrifices the source",
            card.id
        );
        assert!(trigger.program.is_none(), "{} legacy effect list", card.id);
    }
}

#[test]
fn becomes_target_sacrifice_batch_fingerprints_match_the_pinned_oracle_text() {
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
