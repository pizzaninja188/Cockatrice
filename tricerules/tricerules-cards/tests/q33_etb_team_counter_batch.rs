#![allow(unused_imports)]
//! Registry conformance for the queue-v33 entry team counters cohort.
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
        id: "katara,_heroic_healer",
        name: "Katara, Heroic Healer",
        mana_cost: "{4}{W}",
        face_id: "katara_heroic_healer",
        fingerprint: "7440152553f0fa135b97834929db2b9cef353563d7e675e3bbfc753a13cebc5e",
        types: &["Creature", "Human", "Warrior", "Ally"],
        body: Some((2, 3)),
        keywords: &[Keyword::Lifelink],
    },
    Card {
        id: "ridgescale_tusker",
        name: "Ridgescale Tusker",
        mana_cost: "{3}{G}{G}",
        face_id: "ridgescale_tusker",
        fingerprint: "fe376dae285a790277dd727a7ffb24e5fffc30df36a1d66c97a27b04a704d405",
        types: &["Creature", "Pangolin", "Beast"],
        body: Some((5, 5)),
        keywords: &[],
    },
    Card {
        id: "supply_runners",
        name: "Supply Runners",
        mana_cost: "{4}{W}",
        face_id: "supply_runners",
        fingerprint: "fe376dae285a790277dd727a7ffb24e5fffc30df36a1d66c97a27b04a704d405",
        types: &["Creature", "Dog"],
        body: Some((2, 2)),
        keywords: &[],
    },
    Card {
        id: "the_falcon,_sam_wilson",
        name: "The Falcon, Sam Wilson",
        mana_cost: "{4}{W}",
        face_id: "the_falcon_sam_wilson",
        fingerprint: "6e0597ebbafe4c695ba24bcf96f21ef6509c17ff2a66478fcb3ce463138e00fe",
        types: &["Creature", "Human", "Hero"],
        body: Some((3, 3)),
        keywords: &[Keyword::Flying],
    },
];

#[test]
fn q33_etb_team_counter_batch_maps_definitions() {
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
            "katara,_heroic_healer" => {
                assert_eq!(f.triggered_abilities.len(), 1);
                let ability = &f.triggered_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![2])
                );
                assert!(matches!(
                    ability.trigger,
                    TriggerCondition::WhenSelfEntersBattlefield
                ));
                assert!(
                    matches!(ability.effect.as_slice(), [SpellEffectKind::PutCountersAll { counter: CounterKind::PlusOnePlusOne, count: Amount::Fixed(1), filter }] if filter.exclude_self && filter.controller == Some(CreatureScopeController::YouControl))
                );
            }
            "ridgescale_tusker" => {
                assert_eq!(f.triggered_abilities.len(), 1);
                let ability = &f.triggered_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(
                    ability.trigger,
                    TriggerCondition::WhenSelfEntersBattlefield
                ));
                assert!(
                    matches!(ability.effect.as_slice(), [SpellEffectKind::PutCountersAll { counter: CounterKind::PlusOnePlusOne, count: Amount::Fixed(1), filter }] if filter.exclude_self && filter.controller == Some(CreatureScopeController::YouControl))
                );
            }
            "supply_runners" => {
                assert_eq!(f.triggered_abilities.len(), 1);
                let ability = &f.triggered_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(
                    ability.trigger,
                    TriggerCondition::WhenSelfEntersBattlefield
                ));
                assert!(
                    matches!(ability.effect.as_slice(), [SpellEffectKind::PutCountersAll { counter: CounterKind::PlusOnePlusOne, count: Amount::Fixed(1), filter }] if filter.exclude_self && filter.controller == Some(CreatureScopeController::YouControl))
                );
            }
            "the_falcon,_sam_wilson" => {
                assert_eq!(f.triggered_abilities.len(), 1);
                let ability = &f.triggered_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![2])
                );
                assert!(matches!(
                    ability.trigger,
                    TriggerCondition::WhenSelfEntersBattlefield
                ));
                assert!(
                    matches!(ability.effect.as_slice(), [SpellEffectKind::PutCountersAll { counter: CounterKind::PlusOnePlusOne, count: Amount::Fixed(1), filter }] if filter.exclude_self && filter.controller == Some(CreatureScopeController::YouControl))
                );
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q33_etb_team_counter_batch_fingerprints_match_the_pinned_oracle_text() {
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
