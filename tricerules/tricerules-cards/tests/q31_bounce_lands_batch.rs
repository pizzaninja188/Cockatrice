//! Registry conformance for the queue-v31 bounce-land cycle.
use tricerules_cards::primitives::{
    AbilityCost, SpellEffectKind, StaticAbilityDef, TriggerCondition,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "azorius_chancery",
        name: "Azorius Chancery",
        fingerprint: "7f799465402ca5b87d2e8edfeb73b217ee41a0441de27f400d7cd8e7a3cf4cc5",
    },
    Card {
        id: "dimir_aqueduct",
        name: "Dimir Aqueduct",
        fingerprint: "a21638dffb18273eccd1f5a1378fe1bdfbd44ac6c8345908509b09b5cbf384cd",
    },
    Card {
        id: "rakdos_carnarium",
        name: "Rakdos Carnarium",
        fingerprint: "e65601d49d178e315125864e9362a57bad224f687f27ff5a8b4b4519e6986276",
    },
    Card {
        id: "gruul_turf",
        name: "Gruul Turf",
        fingerprint: "809fea889887fa0e32752703be7e020ff07b03359b6854e561cd85f6b3cc1174",
    },
    Card {
        id: "selesnya_sanctuary",
        name: "Selesnya Sanctuary",
        fingerprint: "1dc55d383978384b263e06f2d009f5d1a3469e102e7b5c862f411aa6d4a2bcc6",
    },
    Card {
        id: "orzhov_basilica",
        name: "Orzhov Basilica",
        fingerprint: "7c43753f40988d1da3f9559bd511a7e7a52cf2456a28c713d58ceb158a561775",
    },
    Card {
        id: "izzet_boilerworks",
        name: "Izzet Boilerworks",
        fingerprint: "3a024098b0a082321f8e981ba6a17d47784347498d8cf1e81a8568b7269c39d1",
    },
    Card {
        id: "golgari_rot_farm",
        name: "Golgari Rot Farm",
        fingerprint: "60d21e18b36ff964ff5dbfbef34f94cf08f166c33a4d6b300051e76f0c883b42",
    },
    Card {
        id: "boros_garrison",
        name: "Boros Garrison",
        fingerprint: "66645156b5d70093e6cc2635105957caf549e4110ae5a8f3816ab16024baa5cc",
    },
    Card {
        id: "simic_growth_chamber",
        name: "Simic Growth Chamber",
        fingerprint: "75cd40ccde9f4fc5ef049c45d9ba1ea1818a0cebc2c2c52035686dc4af1395ff",
    },
];

#[test]
fn q31_bounce_lands_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        let f = definition.primary_face();
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        assert_eq!(f.face_id.as_str(), card.id.replace('-', "_"), "{}", card.id);
        assert_eq!(f.name, card.name, "{}", card.id);
        assert_eq!(f.mana_cost.to_string(), "", "{}", card.id);
        assert_eq!(f.types, ["Land"], "{}", card.id);
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);
        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{}: one static ability", card.id);
        };
        assert!(
            matches!(
                static_ability.definition,
                StaticAbilityDef::EntersTapped {
                    affected: tricerules_cards::primitives::EntersTappedAffected::Self_,
                    ..
                }
            ),
            "{}: enters tapped",
            card.id
        );
        let [triggered] = f.triggered_abilities.as_slice() else {
            panic!("{}: one triggered ability", card.id);
        };
        assert_eq!(
            triggered.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{}",
            card.id
        );
        assert!(triggered.effect.is_empty(), "{}: program ability", card.id);
        assert!(triggered.targeting.is_none(), "{}: not a target", card.id);
        let program = triggered
            .program
            .as_ref()
            .unwrap_or_else(|| panic!("{}: program", card.id));
        let debug = format!("{program:?}");
        assert!(
            debug.contains("ChoosePermanents"),
            "{}: chooses on resolution",
            card.id
        );
        assert!(debug.contains("ReturnToOwnersHand"), "{}: returns", card.id);
        assert!(
            debug.contains("PreviousEffectObject"),
            "{}: bound choice",
            card.id
        );
        assert!(
            debug.contains("controller: You"),
            "{}: you control",
            card.id
        );
        assert!(debug.contains("Land"), "{}: land filter", card.id);
        let [mana] = f.activated_abilities.as_slice() else {
            panic!("{}: one mana ability", card.id);
        };
        assert_eq!(mana.costs, [AbilityCost::Tap], "{}", card.id);
        let [SpellEffectKind::ProduceMana { options, .. }] = mana.effect.as_slice() else {
            panic!("{}: produce mana", card.id);
        };
        assert_eq!(options.len(), 1, "{}: exactly two mana", card.id);
    }
}

#[test]
fn q31_bounce_lands_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], card.id.replace('-', "_"));
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
