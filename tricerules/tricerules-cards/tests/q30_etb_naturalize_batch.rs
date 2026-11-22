//! Registry conformance for the queue-v30 optional enters-destroy cohort.
use tricerules_cards::primitives::{
    EffectSubject, PermanentTypeFilter, SpellEffectKind, TargetFilter, TargetKind,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    body: (u32, u32),
    permanent: PermanentTypeFilter,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "war_priest_of_thune",
        name: "War Priest of Thune",
        mana: "{1}{W}",
        body: (2, 2),
        permanent: PermanentTypeFilter::Enchantment,
        fingerprint: "1c8be40df8eea76c6f36d66534ba8551fc74e963e8a807a68ff981a19a142cc7",
    },
    Card {
        id: "enlightened_ascetic",
        name: "Enlightened Ascetic",
        mana: "{1}{W}",
        body: (1, 1),
        permanent: PermanentTypeFilter::Enchantment,
        fingerprint: "1c8be40df8eea76c6f36d66534ba8551fc74e963e8a807a68ff981a19a142cc7",
    },
    Card {
        id: "wild_celebrants",
        name: "Wild Celebrants",
        mana: "{3}{R}{R}",
        body: (5, 3),
        permanent: PermanentTypeFilter::Artifact,
        fingerprint: "bedb0ee57a95a3023f42c342fb1d58e3c40036557fda8b8dfb9444a1cdf3c927",
    },
    Card {
        id: "batterhorn",
        name: "Batterhorn",
        mana: "{4}{R}",
        body: (4, 3),
        permanent: PermanentTypeFilter::Artifact,
        fingerprint: "bedb0ee57a95a3023f42c342fb1d58e3c40036557fda8b8dfb9444a1cdf3c927",
    },
];

#[test]
fn q30_etb_naturalize_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        let f = definition.primary_face();
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        assert_eq!(f.face_id.as_str(), card.id, "{}", card.id);
        assert_eq!(f.name, card.name, "{}", card.id);
        assert_eq!(f.mana_cost.to_string(), card.mana, "{}", card.id);
        assert_eq!(
            (f.power, f.toughness),
            (Some(card.body.0), Some(card.body.1)),
            "{}",
            card.id
        );
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.activated_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);
        let [triggered] = f.triggered_abilities.as_slice() else {
            panic!("{}: exactly one trigger", card.id);
        };
        assert_eq!(
            triggered.trigger,
            tricerules_cards::primitives::TriggerCondition::WhenSelfEntersBattlefield,
            "{}",
            card.id
        );
        assert!(triggered.may, "{}: optional", card.id);
        assert!(triggered.targeting.is_some(), "{}", card.id);
        let [SpellEffectKind::Destroy { subject }] = triggered.effect.as_slice() else {
            panic!("{}: destroy effect", card.id);
        };
        let EffectSubject::Chosen(target) = subject else {
            panic!("{}: chosen target", card.id);
        };
        assert_eq!(
            **target,
            TargetFilter {
                kind: TargetKind::AnyPermanent,
                permanent_types: vec![card.permanent],
                ..TargetFilter::default()
            },
            "{}",
            card.id
        );
    }
}

#[test]
fn q30_etb_naturalize_batch_fingerprints_match_the_pinned_oracle_text() {
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
