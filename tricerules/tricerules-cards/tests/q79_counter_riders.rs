//! Registry conformance for the queue-v79 03-counter-riders cohort.
//!
//! Every card counters a targeted spell and then applies one deterministic rider: three Clue
//! tokens, draw/mill/untap, or the basic landcycling hand ability. Fingerprints are checked
//! after the presentation metadata refresh.

use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, Amount, CardTypeFilter, EffectSubject, SpellEffectKind,
    TargetKind, ZoneCardFilter,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    fingerprint: &'static str,
}

fn cases() -> Vec<Card> {
    vec![
        Card {
            id: "confirm_suspicions",
            name: "Confirm Suspicions",
            mana_cost: "{3}{U}{U}",
            types: &["Instant"],
            fingerprint: "b18d9de284a13630aee67a0ad3e7cd41c24c562aa1aff60ccb390d54d1d72792",
        },
        Card {
            id: "sokkas_haiku",
            name: "Sokka's Haiku",
            mana_cost: "{3}{U}{U}",
            types: &["Instant", "Lesson"],
            fingerprint: "26bf8a8d77ea8258ae7b8e42c76d5cdac117e7f887c54cc950dc52eff65d5ae1",
        },
        Card {
            id: "dismissive_denial",
            name: "Dismissive Denial",
            mana_cost: "{2}{U}{U}",
            types: &["Instant"],
            fingerprint: "238a8ac4c1c7733fd48c2f2c1a3ae2ea8ece3743e2129be903a93e1e6214651b",
        },
    ]
}

fn face<'a>(
    registry: &'a tricerules_cards::CardRegistry,
    id: &str,
) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn q79_counter_riders_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in cases() {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(
            registry.id_for_name(card.name),
            Some(card.id),
            "{}",
            card.id
        );
        let f = face(registry, card.id);
        assert_eq!(f.face_id.as_str(), card.id, "{} face id", card.id);
        assert_eq!(f.name, card.name, "{} name", card.id);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost, "{} mana", card.id);
        assert_eq!(f.types, card.types, "{} types", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);

        match card.id {
            "confirm_suspicions" => {
                assert!(f.targeting.is_none(), "implicit counter target");
                assert!(matches!(
                    f.spell_effect.as_slice(),
                    [
                        SpellEffectKind::CounterTargetSpell { .. },
                        SpellEffectKind::CreateTokens {
                            token,
                            count: Amount::Fixed(3),
                            ..
                        }
                    ] if token == "clue"
                ));
                assert!(f.activated_abilities.is_empty(), "{}", card.id);
            }
            "sokkas_haiku" => {
                assert!(matches!(
                    f.spell_effect.as_slice(),
                    [
                        SpellEffectKind::CounterTargetSpell { .. },
                        SpellEffectKind::Draw {
                            count: Amount::Fixed(1),
                            ..
                        },
                        SpellEffectKind::Mill {
                            count: Amount::Fixed(3),
                            ..
                        },
                        SpellEffectKind::Untap {
                            subject: EffectSubject::Chosen(filter),
                        }
                    ] if filter.kind == TargetKind::AnyPermanent
                        && filter.permanent_types
                            == vec![tricerules_cards::primitives::PermanentTypeFilter::Land]
                ));
                let groups = &f
                    .targeting
                    .as_ref()
                    .expect("two authored target groups")
                    .groups;
                assert_eq!(groups.len(), 2, "{}", card.id);
                assert_eq!(groups[0].effect_indices, [0]);
                assert_eq!(groups[1].effect_indices, [3]);
                assert_eq!((groups[0].min, groups[0].max), (1, 1));
                assert_eq!((groups[1].min, groups[1].max), (1, 1));
                assert!(f.activated_abilities.is_empty(), "{}", card.id);
            }
            "dismissive_denial" => {
                assert!(matches!(
                    f.spell_effect.as_slice(),
                    [SpellEffectKind::CounterTargetSpell { .. }]
                ));
                let [ability] = f.activated_abilities.as_slice() else {
                    panic!("{}: one basic landcycling ability", card.id);
                };
                assert_eq!(ability.ability_id.as_str(), "activated_01", "{}", card.id);
                assert_eq!(ability.source_zone, AbilitySourceZone::Hand, "{}", card.id);
                let [AbilityCost::Mana(mana), AbilityCost::DiscardSelf] = ability.costs.as_slice()
                else {
                    panic!("{}: mana plus discard self", card.id);
                };
                assert_eq!(mana.to_string(), "{2}", "{}", card.id);
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::SearchLibrary {
                        filter: Some(ZoneCardFilter {
                            card_type: Some(CardTypeFilter::BasicLand),
                            ..
                        }),
                        reveal: true,
                        ..
                    }]
                ));
            }
            other => panic!("unexpected card {other}"),
        }
    }
}

#[test]
fn q79_counter_riders_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in cases() {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
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
