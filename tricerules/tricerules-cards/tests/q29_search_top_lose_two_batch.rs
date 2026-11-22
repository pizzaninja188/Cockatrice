//! Registry conformance for the queue-v29 "search for any card, put it on top, lose 2 life"
//! cohort.
use tricerules_cards::primitives::{
    CardSearchZone, InstructionKind, LifeAmount, PlayerRecipient, SearchDestination,
    SearchSelectionBounds, SearchZoneSelection, SpellEffectKind,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
}

const CARDS: &[Card] = &[
    Card {
        id: "imperial_seal",
        name: "Imperial Seal",
        mana: "{B}",
        types: &["Sorcery"],
    },
    Card {
        id: "cruel_tutor",
        name: "Cruel Tutor",
        mana: "{2}{B}",
        types: &["Sorcery"],
    },
    Card {
        id: "vampiric_tutor",
        name: "Vampiric Tutor",
        mana: "{B}",
        types: &["Instant"],
    },
];

#[test]
fn q29_search_top_lose_two_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        let f = definition.primary_face();
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        assert_eq!(f.face_id.as_str(), card.id.replace('-', "_"));
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana);
        assert_eq!(f.types, card.types);
        assert_eq!(definition.layout, Layout::Normal);
        assert!(f.keywords.is_empty());
        assert!(f.static_abilities.is_empty());
        assert!(f.triggered_abilities.is_empty());
        assert!(f.activated_abilities.is_empty());
        assert!(f.spell_effect.is_empty());
        assert!(f.targeting.is_none());
        let program = f
            .spell_program
            .as_ref()
            .unwrap_or_else(|| panic!("{}: spell program", card.id));
        let [search, lose_life] = program.body.steps.as_slice() else {
            panic!("{}: exactly two instructions", card.id);
        };
        assert_eq!(search.id, "search_top", "{}", card.id);
        let InstructionKind::Perform(search_effect) = &search.kind else {
            panic!("{}: search instruction", card.id);
        };
        let SpellEffectKind::SearchLibrary {
            who,
            filter,
            zones,
            destination,
            shuffle,
            reveal,
            selection_bounds,
            ..
        } = search_effect.as_ref()
        else {
            panic!("{}: search library", card.id);
        };
        assert_eq!(*who, PlayerRecipient::Controller, "{}", card.id);
        assert!(filter.is_none(), "{}: any card", card.id);
        assert_eq!(
            *zones,
            SearchZoneSelection::Fixed(vec![CardSearchZone::Library]),
            "{}",
            card.id
        );
        assert_eq!(*destination, SearchDestination::TopOfLibrary, "{}", card.id);
        assert!(*shuffle, "{}", card.id);
        assert!(!*reveal, "{}: the search is private", card.id);
        assert_eq!(
            *selection_bounds,
            Some(SearchSelectionBounds { min: 1, max: 1 }),
            "{}",
            card.id
        );
        assert_eq!(lose_life.id, "lose_life", "{}", card.id);
        let InstructionKind::Perform(lose_effect) = &lose_life.kind else {
            panic!("{}: lose-life instruction", card.id);
        };
        assert_eq!(
            *lose_effect.as_ref(),
            SpellEffectKind::LoseLife {
                amount: LifeAmount::Fixed(2),
                who: PlayerRecipient::Controller,
            },
            "{}",
            card.id
        );
    }
}

#[test]
fn q29_search_top_lose_two_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, expected) in [
        (
            "imperial_seal",
            "Imperial Seal",
            "7385e3c971920475e4c1a0d64fe7252e38ae6cab1368987d0bed7822fc90eb27",
        ),
        (
            "cruel_tutor",
            "Cruel Tutor",
            "7385e3c971920475e4c1a0d64fe7252e38ae6cab1368987d0bed7822fc90eb27",
        ),
        (
            "vampiric_tutor",
            "Vampiric Tutor",
            "7385e3c971920475e4c1a0d64fe7252e38ae6cab1368987d0bed7822fc90eb27",
        ),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id.replace('-', "_"));
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
