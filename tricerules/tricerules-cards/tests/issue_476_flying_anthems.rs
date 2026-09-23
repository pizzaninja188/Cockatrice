//! Complete-card registry evidence for Air Nomad Legacy and Empyrean Eagle.
//!
//! The identities and full Oracle text were checked against the pinned 2026-08-25 Scryfall
//! `oracle_cards` corpus. Air Nomad Legacy creates a Clue directly; its ruling says this is not
//! investigating. Empyrean Eagle's ruling confirms damage remains marked if it leaves. CR 613.1f
//! and 613.1g place current keyword abilities before layer-7 power/toughness modification.

mod common;

use common::FaceExpectation;
use tricerules_cards::primitives::{
    CreatureScopeController, CreatureScopeFilter, SpellEffectKind, StaticAbilityDef,
    TriggerCondition,
};
use tricerules_cards::{Amount, CardFace, CardRegistry, Keyword};

fn face(id: &str) -> &'static CardFace {
    CardRegistry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing reviewed card {id}"))
        .primary_face()
}

fn flying_creatures_you_control(exclude_self: bool) -> CreatureScopeFilter {
    CreatureScopeFilter {
        controller: Some(CreatureScopeController::YouControl),
        required_keyword: Some(Keyword::Flying),
        exclude_self,
        ..CreatureScopeFilter::default()
    }
}

#[test]
fn issue_476_registers_both_complete_standard_identities() {
    let registry = CardRegistry::global();
    for (id, name, mana_cost, types, keywords, power_toughness) in [
        (
            "air_nomad_legacy",
            "Air Nomad Legacy",
            "{W}{U}",
            &["Enchantment"][..],
            &[][..],
            None,
        ),
        (
            "empyrean_eagle",
            "Empyrean Eagle",
            "{1}{W}{U}",
            &["Creature", "Bird", "Spirit"][..],
            &[Keyword::Flying][..],
            Some((2, 3)),
        ),
    ] {
        assert_eq!(registry.id_for_name(name), Some(id), "{name}");
        FaceExpectation {
            id,
            name,
            face_id: id,
            mana_cost,
            types,
            keywords,
            power_toughness,
        }
        .check();
    }
}

#[test]
fn issue_476_air_nomad_creates_a_clue_and_buffs_only_current_flyers() {
    let face = face("air_nomad_legacy");
    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Air Nomad Legacy has exactly one ETB ability");
    };
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(matches!(
        trigger.effect.as_slice(),
        [SpellEffectKind::CreateTokens {
            token,
            count: Amount::Fixed(1),
            ..
        }]
            if token == "clue"
    ));

    let [static_ability] = face.static_abilities.as_slice() else {
        panic!("Air Nomad Legacy has exactly one static anthem");
    };
    assert!(matches!(
        &static_ability.definition,
        StaticAbilityDef::AnthemPt {
            filter,
            delta_power: 1,
            delta_toughness: 1,
            ..
        } if filter == &flying_creatures_you_control(false)
    ));
}

#[test]
fn issue_476_empyrean_eagle_excludes_itself_from_its_flying_anthem() {
    let face = face("empyrean_eagle");
    assert_eq!(face.keywords, [Keyword::Flying]);
    let [static_ability] = face.static_abilities.as_slice() else {
        panic!("Empyrean Eagle has exactly one static anthem");
    };
    assert!(matches!(
        &static_ability.definition,
        StaticAbilityDef::AnthemPt {
            filter,
            delta_power: 1,
            delta_toughness: 1,
            ..
        } if filter == &flying_creatures_you_control(true)
    ));
    assert!(face.triggered_abilities.is_empty());
    assert!(face.activated_abilities.is_empty());
}
