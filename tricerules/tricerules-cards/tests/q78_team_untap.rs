//! Registry conformance for the queue-v78 03-team-untap cohort.
//!
//! Every card untaps all creatures its controller controls at resolution and optionally applies
//! one printed pump or draw rider first. Fingerprints are checked after the metadata refresh.

use tricerules_cards::primitives::{
    Amount, CreatureScopeController, CreatureScopeFilter, MassPlayerSet, PlayerRecipient,
    SpellEffectKind, TargetFilter, TargetKind,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    face_id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    pump: Option<(i32, i32)>,
    draw: bool,
    fingerprint: &'static str,
}

fn cases() -> [Card; 4] {
    [
        Card {
            id: "mobilize",
            face_id: "mobilize",
            name: "Mobilize",
            mana_cost: "{G}",
            types: &["Sorcery"],
            pump: None,
            draw: false,
            fingerprint: "9ee3284679e9871bb7a0809d30b4f9d0673a2c17a084cb63e8dd963937989867",
        },
        Card {
            id: "to_arms!",
            face_id: "to_arms",
            name: "To Arms!",
            mana_cost: "{1}{W}",
            types: &["Instant"],
            pump: None,
            draw: true,
            fingerprint: "1a8fd6b91f666a6323db7b9b6a64cf3a49507c951edc511d74c0288aa4f095a7",
        },
        Card {
            id: "rallying_roar",
            face_id: "rallying_roar",
            name: "Rallying Roar",
            mana_cost: "{2}{W}",
            types: &["Instant"],
            pump: Some((1, 1)),
            draw: false,
            fingerprint: "29de78a810aecfb823f48fc4fd377e8c5a4c78d1b67ba099496253676c6ccca4",
        },
        Card {
            id: "rally_to_battle",
            face_id: "rally_to_battle",
            name: "Rally to Battle",
            mana_cost: "{3}{W}",
            types: &["Instant"],
            pump: Some((1, 3)),
            draw: false,
            fingerprint: "cb8a065e3d666517551f4da339dba9d2eafa0f74ad64211806f6eb84a09cd1b3",
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
fn q78_team_untap_map_definitions() {
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
        assert_eq!(f.face_id.as_str(), card.face_id, "{} face id", card.id);
        assert_eq!(f.name, card.name, "{} name", card.id);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost, "{} mana", card.id);
        assert_eq!(f.types, card.types, "{} types", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.activated_abilities.is_empty(), "{}", card.id);
        assert!(f.targeting.is_none(), "{}", card.id);

        let mut index = 0;
        if let Some((power, toughness)) = card.pump {
            let pump = f
                .spell_effect
                .get(index)
                .unwrap_or_else(|| panic!("{}: missing pump", card.id));
            match pump {
                SpellEffectKind::PumpAll {
                    filter:
                        CreatureScopeFilter {
                            controller: Some(CreatureScopeController::YouControl),
                            ..
                        },
                    power: actual_power,
                    toughness: actual_toughness,
                } => assert_eq!(
                    (*actual_power, *actual_toughness),
                    (power, toughness),
                    "{} pump",
                    card.id
                ),
                other => panic!("{}: unexpected pump {other:?}", card.id),
            }
            index += 1;
        }
        let untap = f
            .spell_effect
            .get(index)
            .unwrap_or_else(|| panic!("{}: missing untap", card.id));
        match untap {
            SpellEffectKind::UntapAll {
                players: MassPlayerSet::Controller,
                filter:
                    TargetFilter {
                        kind: TargetKind::Creature,
                        ..
                    },
            } => {}
            other => panic!("{}: unexpected untap {other:?}", card.id),
        }
        index += 1;
        if card.draw {
            let draw = f
                .spell_effect
                .get(index)
                .unwrap_or_else(|| panic!("{}: missing draw", card.id));
            match draw {
                SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::Fixed(1),
                } => {}
                other => panic!("{}: unexpected draw {other:?}", card.id),
            }
            index += 1;
        }
        assert_eq!(
            f.spell_effect.len(),
            index,
            "{}: exact effect count",
            card.id
        );
    }
}

#[test]
fn q78_team_untap_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], card.face_id, "face id drift for {}", card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
