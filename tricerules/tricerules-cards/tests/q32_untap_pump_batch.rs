//! Registry conformance for the queue-v32 untap-and-pump instant cohort.
use tricerules_cards::primitives::{EffectSubject, SpellEffectKind, TargetKind};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    pump: (i32, i32),
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "gerrards_command",
        name: "Gerrard's Command",
        mana_cost: "{G}{W}",
        pump: (3, 3),
        fingerprint: "bd12481096625489fa742dd4c413cc068308a3d1c8353998f0b41e10df7d3880",
    },
    Card {
        id: "inspirit",
        name: "Inspirit",
        mana_cost: "{2}{W}",
        pump: (2, 4),
        fingerprint: "49452a2c4b544f4ca69aadfe48a388f6775e6abf1d5afe51a06dc7e29a010745",
    },
    Card {
        id: "ornamental_courage",
        name: "Ornamental Courage",
        mana_cost: "{G}",
        pump: (1, 3),
        fingerprint: "ba63e4e2f50dc1be88cfb65d54c6f13a98132cb6b8fe302849b2158c15e9224c",
    },
];

#[test]
fn q32_untap_pump_batch_maps_definitions() {
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
        assert_eq!(f.types, ["Instant"]);
        assert_eq!((f.power, f.toughness), (None, None));
        assert!(f.keywords.is_empty());
        assert!(f.triggered_abilities.is_empty());
        assert!(f.activated_abilities.is_empty());
        assert!(f.static_abilities.is_empty());
        let [untap, pump] = f.spell_effect.as_slice() else {
            panic!("{}: untap then pump", card.id)
        };
        match untap {
            SpellEffectKind::Untap {
                subject: EffectSubject::Chosen(filter),
            } => assert_eq!(filter.kind, TargetKind::Creature, "{}", card.id),
            _ => panic!("{}: untap the chosen creature", card.id),
        }
        assert!(
            matches!(pump, SpellEffectKind::PumpTarget { power, toughness, .. } if *power == card.pump.0 && *toughness == card.pump.1),
            "{}: printed pump",
            card.id
        );
        let targeting = f.targeting.as_ref().expect("targeting");
        assert_eq!(targeting.groups.len(), 1);
        assert_eq!(targeting.groups[0].min, 1);
        assert_eq!(targeting.groups[0].max, 1);
        assert_eq!(targeting.groups[0].effect_indices, [0, 1]);
    }
}

#[test]
fn q32_untap_pump_batch_fingerprints_match_the_pinned_oracle_text() {
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
