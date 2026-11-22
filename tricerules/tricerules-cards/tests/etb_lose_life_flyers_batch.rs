//! Registry conformance for the queue-v26 flying entry life-loss cohort.
//!
//! Every card is a normal-layout flying creature with exactly one mandatory
//! entry trigger that makes its controller lose the printed amount of life.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{LifeAmount, PlayerRecipient, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Keyword, Layout, TriggerCondition};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    loss: u32,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "foul_imp",
        name: "Foul Imp",
        mana_cost: "{B}{B}",
        types: &["Creature", "Imp"],
        power: 2,
        toughness: 2,
        loss: 2,
        fingerprint: "67e34052164f798ef3aa1e16f225067d087e1f393361453a1a29187ed377bcd9",
    },
    Card {
        id: "necrogen_scudder",
        name: "Necrogen Scudder",
        mana_cost: "{2}{B}",
        types: &["Creature", "Phyrexian", "Horror"],
        power: 3,
        toughness: 3,
        loss: 3,
        fingerprint: "0160dbe41e23bc1c062d82964603745f03dc43dd796ce23800e5831194cc2c6d",
    },
    Card {
        id: "dread_reaper",
        name: "Dread Reaper",
        mana_cost: "{3}{B}{B}{B}",
        types: &["Creature", "Horror"],
        power: 6,
        toughness: 5,
        loss: 5,
        fingerprint: "e4d3750e707dfa31b04fb3e541d0077f8d9611a45d93e8716296963f2949b804",
    },
    Card {
        id: "vampiric_spirit",
        name: "Vampiric Spirit",
        mana_cost: "{2}{B}{B}",
        types: &["Creature", "Spirit"],
        power: 4,
        toughness: 3,
        loss: 4,
        fingerprint: "794d50ea0e5464dce25d065ebaf6843e6f92b7e36a502c2fa9b58aa65d04173b",
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
fn etb_lose_life_flyers_batch_maps_definitions() {
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
        assert!(!triggered.may, "{}: the life loss is mandatory", card.id);
        assert!(
            matches!(
                triggered.effect.as_slice(),
                [SpellEffectKind::LoseLife {
                    amount: LifeAmount::Fixed(loss),
                    who: PlayerRecipient::Controller,
                }] if *loss == card.loss
            ),
            "{}: controller loses the printed amount",
            card.id
        );
    }
}

#[test]
fn etb_lose_life_flyers_batch_fingerprints_match_the_pinned_oracle_text() {
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
