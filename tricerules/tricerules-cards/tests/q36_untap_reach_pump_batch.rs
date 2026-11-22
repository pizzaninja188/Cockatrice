//! Registry conformance for the queue-v36 untap-and-reach cohort.
use tricerules_cards::primitives::{EffectSubject, SpellEffectKind};
use tricerules_cards::{Keyword, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    fingerprint: &'static str,
    power: i32,
    toughness: i32,
}

const CARDS: &[Card] = &[
    Card {
        id: "aim_high",
        name: "Aim High",
        mana_cost: "{1}{G}",
        fingerprint: "c60ecb660a150d1f7f6ecb6ef9d5e1369c2fa7ecb9ebc03322949676aa42e602",
        power: 2,
        toughness: 2,
    },
    Card {
        id: "spidery_grasp",
        name: "Spidery Grasp",
        mana_cost: "{2}{G}",
        fingerprint: "22a6331f49a6478ad6c0a5b7a1fa79443ca9a614658419cfa31d27e1ef254168",
        power: 2,
        toughness: 4,
    },
    Card {
        id: "steady_aim",
        name: "Steady Aim",
        mana_cost: "{1}{G}",
        fingerprint: "41345dd5b21c706e4f5ad257a2b31ee3da5ec4c1b6ba559e745a27ec2c36f55a",
        power: 1,
        toughness: 4,
    },
];

#[test]
fn q36_untap_reach_pump_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        assert_eq!(f.face_id.as_str(), card.id);
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost);
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            ["Instant"].to_vec()
        );
        assert_eq!((f.power, f.toughness), (None, None), "{}", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert_eq!(f.spell_effect.len(), 3, "{}", card.id);
        assert!(
            matches!(
                &f.spell_effect[0],
                SpellEffectKind::Untap {
                    subject: EffectSubject::Chosen(filter),
                } if filter.kind == tricerules_cards::primitives::TargetKind::Creature
            ),
            "{}: untap a chosen creature",
            card.id
        );
        assert!(
            matches!(
                &f.spell_effect[1],
                SpellEffectKind::PumpTarget {
                    power,
                    toughness,
                    subject: EffectSubject::Chosen(_),
                    ..
                } if *power == card.power && *toughness == card.toughness
            ),
            "{}: printed pump on the chosen creature",
            card.id
        );
        assert!(
            matches!(
                &f.spell_effect[2],
                SpellEffectKind::GrantKeywords { subject: EffectSubject::Chosen(_), keywords }
                    if keywords == &[Keyword::Reach]
            ),
            "{}: reach on the chosen creature",
            card.id
        );
        let groups = &f.targeting.as_ref().expect("target group").groups;
        assert_eq!(groups.len(), 1, "{}", card.id);
        assert_eq!((groups[0].min, groups[0].max), (1, 1), "{}", card.id);
        assert_eq!(groups[0].effect_indices, [0, 1, 2], "{}", card.id);
    }
}

#[test]
fn q36_untap_reach_pump_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], card.id, "face id drift for {}", card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
