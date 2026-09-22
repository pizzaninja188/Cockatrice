//! Pinned Oracle identities 7fe5baab-b182-4cd9-bea5-912ce9b1464a and
//! ecc4e230-a546-4cb4-987c-87300060bbe2. The other #444 Commands remain
//! unregistered while #445 and #446 own their final printed bullets.

use tricerules_cards::primitives::{
    CreatureScopeController, RelativePlayerSet, SpellEffectKind, TargetKind,
};
use tricerules_cards::{AbilityPresentation, CardRegistry};

#[test]
fn issue_444_complete_commands_have_exact_four_mode_presentation() {
    let registry = CardRegistry::global();
    for (id, name, face_id, mana, subtype) in [
        (
            "syggs_command",
            "Sygg's Command",
            "sygg_s_command",
            "{1}{W}{U}",
            "Merfolk",
        ),
        (
            "trystans_command",
            "Trystan's Command",
            "trystan_s_command",
            "{4}{B}{G}",
            "Elf",
        ),
    ] {
        let card = registry.get(id).expect("complete Command registered");
        assert_eq!(card.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        let face = card.primary_face();
        assert_eq!(face.face_id.as_str(), face_id);
        assert_eq!(face.mana_cost.to_string(), mana);
        assert_eq!(
            face.types.iter().map(String::as_str).collect::<Vec<_>>(),
            ["Kindred", "Sorcery", subtype]
        );
        assert!(face.spell_effect.is_empty() && face.targeting.is_none());
        let modal = face.modal_spell.as_ref().expect("modal spell");
        assert_eq!(
            (modal.min_modes, modal.max_modes, modal.modes.len()),
            (2, 2, 4)
        );
        for (index, mode) in modal.modes.iter().enumerate() {
            assert_eq!(mode.mode_id.as_str(), format!("mode_{:02}", index + 1));
            assert_eq!(
                mode.presentation,
                AbilityPresentation::OracleLines(vec![index as u16 + 2])
            );
        }
    }
    for excluded in ["ashlings_command", "grubs_command"] {
        assert!(registry.get(excluded).is_none());
    }
}

#[test]
fn issue_444_mass_modes_bind_one_player_and_keep_ordered_instructions() {
    let registry = CardRegistry::global();
    let sygg = registry
        .get("syggs_command")
        .unwrap()
        .primary_face()
        .modal_spell
        .as_ref()
        .unwrap();
    let trystan = registry
        .get("trystans_command")
        .unwrap()
        .primary_face()
        .modal_spell
        .as_ref()
        .unwrap();
    assert!(
        matches!(&sygg.modes[1].effects[..], [SpellEffectKind::GrantKeywordsAll { filter, keywords }]
        if matches!(filter.controller, Some(CreatureScopeController::TargetedPlayer { group_index: 0, kind: TargetKind::AnyPlayer }))
            && keywords == &[tricerules_cards::Keyword::Lifelink])
    );
    assert!(
        matches!(&trystan.modes[3].effects[..], [SpellEffectKind::PumpAll { filter, power: 3, toughness: 3 }, SpellEffectKind::UntapAll { players: RelativePlayerSet::TargetedPlayer { group_index: 0, kind: TargetKind::AnyPlayer }, .. }]
        if matches!(filter.controller, Some(CreatureScopeController::TargetedPlayer { group_index: 0, kind: TargetKind::AnyPlayer })))
    );
    for mode in [&sygg.modes[1], &trystan.modes[3]] {
        let group = &mode
            .targeting
            .as_ref()
            .expect("authored target group")
            .groups[0];
        assert_eq!((group.min, group.max), (1, 1));
        assert_eq!(group.effect_indices.len(), mode.effects.len());
        assert_eq!(group.prompt, "Choose target player");
    }
    let return_group = &trystan.modes[1].targeting.as_ref().unwrap().groups[0];
    assert_eq!((return_group.min, return_group.max), (1, 2));
}
