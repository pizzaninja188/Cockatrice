//! Registry identity for the Auras that grant "{cost}: Regenerate this creature." (issue #522):
//! Molting Snakeskin, Savage Silhouette and Trollhide. Names, costs, pumps and granted costs are
//! the printed Oracle values; the shared scenario module executes every card.
use crate::common;

use common::FaceExpectation;
use tricerules_cards::primitives::{
    AbilityCost, EffectSubject, SpellEffectKind, StaticAbilityDef, TargetKind,
};
use tricerules_cards::AbilityPresentation;

#[test]
fn issue_522_registers_granted_regenerate_auras() {
    for (id, name, mana_cost, pump, granted_cost) in [
        (
            "molting_snakeskin",
            "Molting Snakeskin",
            "{B}",
            (2, 0),
            "{2}{B}",
        ),
        (
            "savage_silhouette",
            "Savage Silhouette",
            "{2}{G}",
            (2, 2),
            "{1}{G}",
        ),
        ("trollhide", "Trollhide", "{2}{G}", (2, 2), "{1}{G}"),
    ] {
        let face = FaceExpectation {
            id,
            name,
            face_id: id,
            mana_cost,
            types: &["Enchantment", "Aura"],
            keywords: &[],
            power_toughness: None,
        }
        .check();
        let [SpellEffectKind::AuraAttach { target }] = face.spell_effect.as_slice() else {
            panic!("{id}: exactly one AuraAttach spell effect");
        };
        assert_eq!(target.kind, TargetKind::Creature, "{id}: enchant creature");
        assert!(face.activated_abilities.is_empty(), "{id}");
        assert!(face.triggered_abilities.is_empty(), "{id}");
        assert!(face.spell_program.is_none(), "{id}");

        let [static_ability] = face.static_abilities.as_slice() else {
            panic!("{id}: exactly one attached static modifier");
        };
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id}"
        );
        let StaticAbilityDef::AttachedModifier {
            delta_power,
            delta_toughness,
            keywords,
            activated_abilities,
            ..
        } = &static_ability.definition
        else {
            panic!("{id}: attached modifier");
        };
        assert_eq!((*delta_power, *delta_toughness), pump, "{id}: pump");
        assert!(keywords.is_empty(), "{id}");
        let [granted] = activated_abilities.as_slice() else {
            panic!("{id}: exactly one granted activated ability");
        };
        let [AbilityCost::Mana(cost)] = granted.costs.as_slice() else {
            panic!("{id}: mana-only granted cost");
        };
        assert_eq!(cost.to_string(), granted_cost, "{id}: granted cost");
        assert_eq!(
            granted.effect,
            [SpellEffectKind::Regenerate {
                subject: EffectSubject::Source
            }],
            "{id}: regenerate the enchanted creature"
        );
        assert!(granted.targeting.is_none(), "{id}");
        assert_eq!(
            granted.fallback_text("Grizzly Bears"),
            format!("{granted_cost}: Regenerate this creature."),
            "{id}: exact printed activation line"
        );
    }
}
