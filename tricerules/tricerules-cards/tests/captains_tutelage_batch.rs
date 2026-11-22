//! Registry conformance for the reviewed Captain's Tutelage Aura family.
//!
//! Every included card is a normal-layout Aura with Flash, `AuraAttach()`, one entry
//! trigger that grants first strike to the attached creature, and one attached P/T
//! modifier. Card names, costs and numbers are printed data; the shared scenario
//! executes every card. Each fingerprint is SHA-256 of the exact pinned normalized
//! Oracle text.

use tricerules_cards::primitives::{
    SpellEffectKind, StaticAbilityDef, TargetController, TargetKind,
};
use tricerules_cards::{AbilityPresentation, Keyword, Layout, TriggerCondition};

type Card = (&'static str, &'static str, &'static str, i32);

const CARDS: &[Card] = &[
    ("aspect_of_manticore", "Aspect of Manticore", "{2}{R}", 2),
    ("captains_tutelage", "Captain's Tutelage", "{1}{R}", 2),
    ("military_discipline", "Military Discipline", "{W}", 1),
];

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
fn captains_tutelage_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, delta_power) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}: name lookup");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, ["Enchantment", "Aura"], "{id} types");
        assert_eq!(f.power, None, "{id} has no power");
        assert_eq!(f.toughness, None, "{id} has no toughness");
        assert_eq!(f.keywords, [Keyword::Flash], "{id} flash");
        assert!(
            f.activated_abilities.is_empty(),
            "{id} has no activated ability"
        );

        let [attach] = f.spell_effect.as_slice() else {
            panic!("{id}: exactly one Aura attach effect");
        };
        assert!(
            matches!(
                attach,
                SpellEffectKind::AuraAttach { target }
                    if target.kind == TargetKind::Creature
                        && target.controller == TargetController::Any
            ),
            "{id}: {:?}",
            f.spell_effect
        );

        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one entry trigger");
        };
        assert_eq!(trigger.ability_id.as_str(), "triggered_01", "{id}");
        assert_eq!(
            trigger.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![3]),
            "{id} trigger presentation"
        );
        assert!(
            matches!(
                trigger.effect.as_slice(),
                [SpellEffectKind::GrantKeywords {
                    subject: tricerules_cards::primitives::EffectSubject::AttachedObject,
                    keywords,
                }] if keywords == &[Keyword::FirstStrike]
            ),
            "{id}: attached-object first strike"
        );

        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{id}: exactly one static ability");
        };
        assert_eq!(static_ability.ability_id.as_str(), "static_01", "{id}");
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![4]),
            "{id} static presentation"
        );
        assert!(
            matches!(
                static_ability.definition,
                StaticAbilityDef::AttachedModifier {
                    delta_power: power,
                    delta_toughness: 0,
                    ..
                } if power == delta_power
            ),
            "{id}: +{delta_power}/+0 attached modifier"
        );
    }
}

#[test]
fn captains_tutelage_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _) in CARDS {
        let expected = match id {
            "aspect_of_manticore" => {
                "c796d239581c336ae6df3d682cec57e925be7d4d68258a793a8c0815383ec12e"
            }
            "captains_tutelage" => {
                "c796d239581c336ae6df3d682cec57e925be7d4d68258a793a8c0815383ec12e"
            }
            "military_discipline" => {
                "865c4b1456f77916bd607b9076ad7742837d1b0c9e52e751671f6a175a1fd4a4"
            }
            _ => unreachable!(),
        };
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id);
        assert_eq!(fields[3], name);
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
