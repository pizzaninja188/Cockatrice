//! Registry conformance for the queue-v32 Elder land cycle.
use tricerules_cards::primitives::{AbilityCost, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    single: char,
    pair: [char; 2],
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "an-havva_township",
        name: "An-Havva Township",
        single: 'g',
        pair: ['r', 'w'],
        fingerprint: "ec234e9aa0c57e822607be56decdc3cf5b23b9edccd5a6f8d6a3aa44da86793d",
    },
    Card {
        id: "aysen_abbey",
        name: "Aysen Abbey",
        single: 'w',
        pair: ['g', 'u'],
        fingerprint: "ea2ce9f9e2c845278f3c055b7dbd737d4e5252db614c11ac6ff6e81be1b42fc4",
    },
    Card {
        id: "castle_sengir",
        name: "Castle Sengir",
        single: 'b',
        pair: ['u', 'r'],
        fingerprint: "4971153a5fea6ff9dd50383ea042ac61ed4b697ba94f9fb6f6309350cd13f543",
    },
    Card {
        id: "koskun_keep",
        name: "Koskun Keep",
        single: 'r',
        pair: ['b', 'g'],
        fingerprint: "d505a7f4cdbe3ce9ca132cdc79b9091513c5efeca8585ed2263f0f8bb7e6c43a",
    },
    Card {
        id: "wizards_school",
        name: "Wizards' School",
        single: 'u',
        pair: ['w', 'b'],
        fingerprint: "89f4fd8897824e7492e0708506efdcc1c84a3eff71f0a2ebad9f57687f5ac84c",
    },
];

fn mask(color: char) -> [u32; 6] {
    match color {
        'w' => [1, 0, 0, 0, 0, 0],
        'u' => [0, 1, 0, 0, 0, 0],
        'b' => [0, 0, 1, 0, 0, 0],
        'r' => [0, 0, 0, 1, 0, 0],
        'g' => [0, 0, 0, 0, 1, 0],
        _ => unreachable!("printed color"),
    }
}

fn amounts(amount: &tricerules_cards::ManaAmount) -> [u32; 6] {
    [amount.w, amount.u, amount.b, amount.r, amount.g, amount.c]
}

#[test]
fn q32_elder_lands_batch_maps_definitions() {
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
        assert_eq!(f.mana_cost.to_string(), "");
        assert_eq!(f.types, ["Land"]);
        assert_eq!((f.power, f.toughness), (None, None));
        assert!(f.keywords.is_empty());
        assert!(f.triggered_abilities.is_empty());
        assert!(f.spell_effect.is_empty());
        assert!(f.static_abilities.is_empty());
        assert!(f.targeting.is_none());
        let [colorless, single, pair] = f.activated_abilities.as_slice() else {
            panic!("{}: three activated abilities", card.id)
        };
        assert_eq!(
            colorless.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(colorless.costs, [AbilityCost::Tap]);
        assert_eq!(
            single.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert!(
            matches!(&single.costs[..], [AbilityCost::Mana(cost), AbilityCost::Tap] if cost.to_string() == "{1}")
        );
        assert_eq!(pair.presentation, AbilityPresentation::OracleLines(vec![3]));
        assert!(
            matches!(&pair.costs[..], [AbilityCost::Mana(cost), AbilityCost::Tap] if cost.to_string() == "{2}")
        );
        assert!(
            matches!(
                colorless.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 1 && amounts(&options[0]) == [0, 0, 0, 0, 0, 1]
            ),
            "{}: colorless",
            card.id
        );
        assert!(
            matches!(
                single.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 1 && amounts(&options[0]) == mask(card.single)
            ),
            "{}: single color",
            card.id
        );
        assert!(
            matches!(
                pair.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 2
                        && amounts(&options[0]) == mask(card.pair[0])
                        && amounts(&options[1]) == mask(card.pair[1])
            ),
            "{}: two-color option",
            card.id
        );
    }
}

#[test]
fn q32_elder_lands_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
