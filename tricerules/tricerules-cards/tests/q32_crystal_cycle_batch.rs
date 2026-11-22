//! Registry conformance for the queue-v32 Crystal artifact cycle.
use tricerules_cards::primitives::{AbilityCost, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, AbilitySourceZone, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    colors: [char; 3],
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "indatha_crystal",
        name: "Indatha Crystal",
        colors: ['w', 'b', 'g'],
        fingerprint: "47b292795cb8379f77c59c5e7b8796c1e8370996d99067604657a1863bbe6763",
    },
    Card {
        id: "ketria_crystal",
        name: "Ketria Crystal",
        colors: ['g', 'u', 'r'],
        fingerprint: "ac32ce12a5a057bece1fa09b1e0d559393856cc0d13d5ba55111421df16c576c",
    },
    Card {
        id: "raugrin_crystal",
        name: "Raugrin Crystal",
        colors: ['u', 'r', 'w'],
        fingerprint: "7870fbbea9f4cbf920c41a218d13abe7effbd7a2684e19fa2543d4db664f852f",
    },
    Card {
        id: "savai_crystal",
        name: "Savai Crystal",
        colors: ['r', 'w', 'b'],
        fingerprint: "5a97673522c586d3326d5602929490c88de8477fbe559d3c423d13aeddb30e54",
    },
    Card {
        id: "zagoth_crystal",
        name: "Zagoth Crystal",
        colors: ['b', 'g', 'u'],
        fingerprint: "d8e278bd4bb008b6f00520dc6f3a1e68d46bc1aea15221a4ce4b1332a3910c5b",
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
fn q32_crystal_cycle_batch_maps_definitions() {
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
        assert_eq!(f.mana_cost.to_string(), "{3}");
        assert_eq!(f.types, ["Artifact"]);
        assert_eq!((f.power, f.toughness), (None, None));
        assert!(f.keywords.is_empty());
        assert!(f.triggered_abilities.is_empty());
        assert!(f.spell_effect.is_empty());
        assert!(f.static_abilities.is_empty());
        assert!(f.targeting.is_none());
        let [mana, cycling] = f.activated_abilities.as_slice() else {
            panic!("{}: two activated abilities", card.id)
        };
        assert_eq!(mana.presentation, AbilityPresentation::OracleLines(vec![1]));
        assert_eq!(mana.costs, [AbilityCost::Tap]);
        assert!(mana.is_mana_ability());
        assert!(
            matches!(
                mana.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 3
                        && amounts(&options[0]) == mask(card.colors[0])
                        && amounts(&options[1]) == mask(card.colors[1])
                        && amounts(&options[2]) == mask(card.colors[2])
            ),
            "{}: three printed options",
            card.id
        );
        assert_eq!(
            cycling.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert_eq!(cycling.source_zone, AbilitySourceZone::Hand);
        assert!(
            matches!(&cycling.costs[..], [AbilityCost::Mana(cost), AbilityCost::DiscardSelf] if cost.to_string() == "{2}"),
            "{}: cycling cost",
            card.id
        );
        assert!(
            matches!(cycling.effect.as_slice(), [SpellEffectKind::Draw { count, .. }] if *count == tricerules_cards::primitives::Amount::Fixed(1)),
            "{}: cycling draw",
            card.id
        );
    }
}

#[test]
fn q32_crystal_cycle_batch_fingerprints_match_the_pinned_oracle_text() {
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
