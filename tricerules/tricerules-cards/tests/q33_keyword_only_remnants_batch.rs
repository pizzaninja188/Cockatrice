#![allow(unused_imports)]
//! Registry conformance for the queue-v33 printed keyword only cohort.
use tricerules_cards::primitives::{
    AbilityCost, Amount, CardTypeFilter, CastTriggerPlayer, Color, CounterKind,
    CreatureScopeController, EffectSubject, RelativePlayerSet, SpellEffectKind, StaticAbilityDef,
    TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, AbilitySourceZone, Keyword, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    face_id: &'static str,
    fingerprint: &'static str,
    types: &'static [&'static str],
    body: Option<(u32, u32)>,
    keywords: &'static [Keyword],
}

const CARDS: &[Card] = &[
    Card {
        id: "alaborn_musketeer",
        name: "Alaborn Musketeer",
        mana_cost: "{1}{W}",
        face_id: "alaborn_musketeer",
        fingerprint: "0eeb291d4d093ec7ed57cf6c7345e4e1692a6c2387f9ab4f941f60aefbe56b16",
        types: &["Creature", "Human", "Soldier"],
        body: Some((2, 1)),
        keywords: &[Keyword::Reach],
    },
    Card {
        id: "kolinahr_priest",
        name: "Kolinahr Priest",
        mana_cost: "{2}{U}",
        face_id: "kolinahr_priest",
        fingerprint: "84d4d83fa8df8fb1dcc80868f820c63713c5ebf1c5c4f36c5c0a1c350331ae91",
        types: &["Creature", "Vulcan", "Monk"],
        body: Some((3, 2)),
        keywords: &[Keyword::Flash],
    },
    Card {
        id: "mekleth_berserker",
        name: "Mek'leth Berserker",
        mana_cost: "{3}{R}",
        face_id: "mekleth_berserker",
        fingerprint: "1a3c963b3c089d7d6a6ec3848e8caff20dfc0c3706e3406f422d1ab4c9d5cfae",
        types: &["Creature", "Klingon", "Berserker"],
        body: Some((4, 3)),
        keywords: &[Keyword::Menace],
    },
    Card {
        id: "sickbay_orderly",
        name: "Sickbay Orderly",
        mana_cost: "{2}{W}",
        face_id: "sickbay_orderly",
        fingerprint: "d671d3cbff8e25915c020bc067e36e088a74e1c459ac0a62be8d6c9b8222c240",
        types: &["Creature", "Caitian", "Doctor"],
        body: Some((3, 2)),
        keywords: &[Keyword::Lifelink],
    },
    Card {
        id: "steel_wall",
        name: "Steel Wall",
        mana_cost: "{1}",
        face_id: "steel_wall",
        fingerprint: "82fdab220543c360845cafccf126965515be4b0c831d0392f9aca2221b4c921d",
        types: &["Artifact", "Creature", "Wall"],
        body: Some((0, 4)),
        keywords: &[Keyword::Defender],
    },
    Card {
        id: "talarian_hook_spider",
        name: "Talarian Hook Spider",
        mana_cost: "{1}{G}",
        face_id: "talarian_hook_spider",
        fingerprint: "0eeb291d4d093ec7ed57cf6c7345e4e1692a6c2387f9ab4f941f60aefbe56b16",
        types: &["Creature", "Spider"],
        body: Some((2, 2)),
        keywords: &[Keyword::Reach],
    },
    Card {
        id: "worf,_chief_tactical_officer",
        name: "Worf, Chief Tactical Officer",
        mana_cost: "{2}{W}",
        face_id: "worf_chief_tactical_officer",
        fingerprint: "713185635adb649f365b07ed8e8b0bd39a7dc84b34a41ece29fb179fdd2086ef",
        types: &["Creature", "Klingon", "Warrior", "Officer"],
        body: Some((2, 3)),
        keywords: &[Keyword::Vigilance],
    },
];

#[test]
fn q33_keyword_only_remnants_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        assert_eq!(f.face_id.as_str(), card.face_id);
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost);
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            card.types.to_vec()
        );
        match card.body {
            Some((power, toughness)) => {
                assert_eq!((f.power, f.toughness), (Some(power), Some(toughness)))
            }
            None => assert_eq!((f.power, f.toughness), (None, None)),
        }
        assert_eq!(f.keywords, card.keywords);
        match card.id {
            "alaborn_musketeer" => {
                assert!(f.activated_abilities.is_empty());
                assert!(f.triggered_abilities.is_empty());
                assert!(f.static_abilities.is_empty());
                assert!(f.spell_effect.is_empty());
            }
            "kolinahr_priest" => {
                assert!(f.activated_abilities.is_empty());
                assert!(f.triggered_abilities.is_empty());
                assert!(f.static_abilities.is_empty());
                assert!(f.spell_effect.is_empty());
            }
            "mekleth_berserker" => {
                assert!(f.activated_abilities.is_empty());
                assert!(f.triggered_abilities.is_empty());
                assert!(f.static_abilities.is_empty());
                assert!(f.spell_effect.is_empty());
            }
            "sickbay_orderly" => {
                assert!(f.activated_abilities.is_empty());
                assert!(f.triggered_abilities.is_empty());
                assert!(f.static_abilities.is_empty());
                assert!(f.spell_effect.is_empty());
            }
            "steel_wall" => {
                assert!(f.activated_abilities.is_empty());
                assert!(f.triggered_abilities.is_empty());
                assert!(f.static_abilities.is_empty());
                assert!(f.spell_effect.is_empty());
            }
            "talarian_hook_spider" => {
                assert!(f.activated_abilities.is_empty());
                assert!(f.triggered_abilities.is_empty());
                assert!(f.static_abilities.is_empty());
                assert!(f.spell_effect.is_empty());
            }
            "worf,_chief_tactical_officer" => {
                assert!(f.activated_abilities.is_empty());
                assert!(f.triggered_abilities.is_empty());
                assert!(f.static_abilities.is_empty());
                assert!(f.spell_effect.is_empty());
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q33_keyword_only_remnants_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], card.face_id, "face id drift for {}", card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
