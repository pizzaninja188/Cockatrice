//! Registry conformance for the queue-v39 land entry-target cohort.
use tricerules_cards::primitives::{Keyword, SpellEffectKind, StaticAbilityDef, TriggerCondition};
use tricerules_cards::{AbilityPresentation, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    power: i32,
    toughness: i32,
    keywords: &'static [Keyword],
}

const CARDS: &[Card] = &[
    Card {
        id: "sandstone_bridge",
        name: "Sandstone Bridge",
        power: 1,
        toughness: 1,
        keywords: &[Keyword::Vigilance],
    },
    Card {
        id: "teetering_peaks",
        name: "Teetering Peaks",
        power: 2,
        toughness: 0,
        keywords: &[],
    },
    Card {
        id: "turntimber_grove",
        name: "Turntimber Grove",
        power: 1,
        toughness: 1,
        keywords: &[],
    },
    Card {
        id: "looming_spires",
        name: "Looming Spires",
        power: 1,
        toughness: 1,
        keywords: &[Keyword::FirstStrike],
    },
    Card {
        id: "soaring_seacliff",
        name: "Soaring Seacliff",
        power: 0,
        toughness: 0,
        keywords: &[Keyword::Flying],
    },
];

#[test]
fn q39_land_etb_target_maps_definitions() {
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
        assert_eq!(f.types, ["Land"]);
        assert_eq!((f.power, f.toughness), (None, None));
        assert!(f.keywords.is_empty());
        assert!(f.spell_effect.is_empty());
        assert_eq!(f.static_abilities.len(), 1);
        assert!(matches!(
            f.static_abilities[0].definition,
            StaticAbilityDef::EntersTapped { .. }
        ));
        assert_eq!(f.triggered_abilities.len(), 1);
        let trigger = &f.triggered_abilities[0];
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert_eq!(trigger.targeting.as_ref().map(|t| t.groups.len()), Some(1));
        assert_eq!(f.activated_abilities.len(), 1);
        let mana = &f.activated_abilities[0];
        assert_eq!(mana.presentation, AbilityPresentation::OracleLines(vec![3]));
        assert!(matches!(
            mana.effect.as_slice(),
            [SpellEffectKind::ProduceMana { .. }]
        ));
        if card.id != "soaring_seacliff" {
            let pumped = trigger.effect.iter().any(|effect| {
                matches!(
                    effect,
                    SpellEffectKind::PumpTarget { power, toughness, .. }
                        if *power == card.power && *toughness == card.toughness
                )
            });
            assert!(pumped, "{}: printed P/T effect", card.id);
        }
        for keyword in card.keywords {
            assert!(
                trigger.effect.iter().any(|effect| matches!(
                    effect,
                    SpellEffectKind::GrantKeywords { keywords, .. } if keywords.contains(keyword)
                )),
                "{}: missing printed keyword {:?}",
                card.id,
                keyword
            );
        }
    }
}
