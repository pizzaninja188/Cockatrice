//! Registry conformance for the queue-v38 01-creature-player-sweep cohort.
//! Printed identity, costs, typed shape and pinned fingerprints.

use tricerules_cards::primitives::{Amount, PlayerRecipient, SpellEffectKind, TargetKind};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: Option<u32>,
    toughness: Option<u32>,
    amount: u32,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "dakmor_plague",
        name: "Dakmor Plague",
        mana_cost: "{3}{B}{B}",
        types: &["Sorcery"],
        power: None,
        toughness: None,
        amount: 3,
        fingerprint: "b5ee2d03af75fe882cd1db66c9e3ca34b07911c5260b2045838af68d65140a76",
    },
    Card {
        id: "dry_spell",
        name: "Dry Spell",
        mana_cost: "{1}{B}",
        types: &["Sorcery"],
        power: None,
        toughness: None,
        amount: 1,
        fingerprint: "0b16366651659f3ec50597c84f907b3b313bb07a1a6e1a968f6bf6e20b555bcf",
    },
    Card {
        id: "famine",
        name: "Famine",
        mana_cost: "{3}{B}{B}",
        types: &["Sorcery"],
        power: None,
        toughness: None,
        amount: 3,
        fingerprint: "04596410c5a97d420161b3cf6cc538ba7eaa495ef4c8109a68a88c3aa59a3666",
    },
    Card {
        id: "fire_tempest",
        name: "Fire Tempest",
        mana_cost: "{5}{R}{R}",
        types: &["Sorcery"],
        power: None,
        toughness: None,
        amount: 6,
        fingerprint: "a98a0bb6ff3bc08047e1a8b48c8753ec4407219e1e94a23b16d2db35332825e5",
    },
    Card {
        id: "rain_of_embers",
        name: "Rain of Embers",
        mana_cost: "{1}{R}",
        types: &["Sorcery"],
        power: None,
        toughness: None,
        amount: 1,
        fingerprint: "625c9e30fe556c4a85cbea2a141f0006a4f2db24339fb561d40b8f0e05c09dc6",
    },
    Card {
        id: "steam_blast",
        name: "Steam Blast",
        mana_cost: "{2}{R}",
        types: &["Sorcery"],
        power: None,
        toughness: None,
        amount: 2,
        fingerprint: "d7fa11659dc5c9153d10c042471447a6dc8811c30aa496ec300c89d9cb35ebac",
    },
];

use tricerules_cards::Layout;
#[test]
fn q38_creature_player_sweep_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        assert_eq!(f.face_id.as_str(), card.id.replace('-', "_"));
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost);
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            card.types
        );
        assert_eq!((f.power, f.toughness), (card.power, card.toughness));
        assert!(f.keywords.is_empty());
        assert!(f.static_abilities.is_empty());
        assert!(f.triggered_abilities.is_empty());
        assert!(f.activated_abilities.is_empty());
        assert!(f.targeting.is_none());
        assert!(matches!(
            f.spell_effect.as_slice(),
            [SpellEffectKind::DamageAll { amount: Amount::Fixed(damage), kind, .. },
             SpellEffectKind::DamagePlayer { amount: Amount::Fixed(player_damage), who: PlayerRecipient::EachPlayer }]
                if *damage == card.amount
                    && *player_damage == card.amount
                    && kind.kind == TargetKind::Creature
        ));
    }
}

#[test]
fn q38_creature_player_sweep_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "{}", card.id);
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(fields[2], card.id.replace('-', "_"));
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
