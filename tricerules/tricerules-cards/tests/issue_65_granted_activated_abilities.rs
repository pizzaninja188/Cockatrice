use tricerules_cards::primitives::{
    AbilityCost, Amount, PermanentTypeFilter, SpellEffectKind, StaticAbilityDef, TargetFilter,
    TargetKind,
};
use tricerules_cards::{CardRegistry, ManaAmount};

fn granted_ability(card_id: &str) -> &tricerules_cards::ActivatedAbilityDef {
    let definition = CardRegistry::global()
        .get(card_id)
        .unwrap_or_else(|| panic!("{card_id} must be registered"));
    let modifier = definition
        .primary_face()
        .static_abilities
        .iter()
        .find_map(|ability| match ability {
            StaticAbilityDef::AttachedModifier {
                activated_abilities,
                ..
            } => Some(activated_abilities),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{card_id} must grant an attached activated ability"));
    assert_eq!(modifier.len(), 1, "{card_id}");
    &modifier[0]
}

#[test]
fn gift_of_paradise_grants_the_exact_land_mana_ability() {
    let definition = CardRegistry::global()
        .get("gift_of_paradise")
        .expect("Gift of Paradise must be registered");
    let face = definition.primary_face();
    assert_eq!(definition.name, "Gift of Paradise");
    assert_eq!(face.mana_cost.to_string(), "{2}{G}");
    assert_eq!(face.types, ["Enchantment", "Aura"]);
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::AuraAttach {
            target: TargetFilter {
                kind: TargetKind::AnyPermanent,
                permanent_types: vec![PermanentTypeFilter::Land],
                ..TargetFilter::default()
            },
        }]
    );
    assert!(face.triggered_abilities.iter().any(|ability| {
        ability.text == "When Gift of Paradise enters, you gain 3 life."
            && ability.effect
                == [SpellEffectKind::GainLife {
                    amount: Amount::Fixed(3),
                }]
    }));

    let ability = granted_ability("gift_of_paradise");
    assert_eq!(ability.costs, [AbilityCost::Tap]);
    let [SpellEffectKind::ProduceMana { options, .. }] = ability.effect.as_slice() else {
        panic!("Gift must grant one mana-production effect");
    };
    assert_eq!(
        options,
        &vec![
            ManaAmount {
                w: 2,
                ..Default::default()
            },
            ManaAmount {
                u: 2,
                ..Default::default()
            },
            ManaAmount {
                b: 2,
                ..Default::default()
            },
            ManaAmount {
                r: 2,
                ..Default::default()
            },
            ManaAmount {
                g: 2,
                ..Default::default()
            },
        ]
    );
    assert_eq!(ability.text, "{T}: Add two mana of any one color.");
}

#[test]
fn hermetic_study_grants_a_targeted_damage_ability() {
    let definition = CardRegistry::global()
        .get("hermetic_study")
        .expect("Hermetic Study must be registered");
    let face = definition.primary_face();
    assert_eq!(definition.name, "Hermetic Study");
    assert_eq!(face.mana_cost.to_string(), "{1}{U}");
    assert_eq!(face.types, ["Enchantment", "Aura"]);

    let ability = granted_ability("hermetic_study");
    assert_eq!(ability.costs, [AbilityCost::Tap]);
    assert_eq!(
        ability.effect,
        [SpellEffectKind::DamageTarget {
            amount: Amount::Fixed(1),
            target: TargetFilter {
                kind: TargetKind::AnyTarget,
                ..TargetFilter::default()
            },
        }]
    );
    assert_eq!(
        ability.text,
        "{T}: This creature deals 1 damage to any target."
    );
}
