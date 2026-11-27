//! #445: Ashling's Command (9651d197-a408-4665-a292-58d5471f4ffc) and Brigid's Command
//! (6487fc1b-5c83-4d09-94d3-175725bcb922). Each "Target player creates ..." bullet binds the
//! token recipient to that mode's own single player target group.

use tricerules_cards::primitives::{MassPlayerSet, PlayerRecipient, SpellEffectKind, TargetKind};
use tricerules_cards::AbilityPresentation;

fn targeted_recipient() -> PlayerRecipient {
    PlayerRecipient::TargetedPlayer {
        group_index: 0,
        kind: TargetKind::AnyPlayer,
    }
}

#[test]
fn issue_445_commands_have_exact_identity_and_four_printed_modes() {
    let registry = tricerules_cards::registry::global();
    for (id, name, face_id, mana, card_type, subtype) in [
        (
            "ashlings_command",
            "Ashling's Command",
            "ashling_s_command",
            "{3}{U}{R}",
            "Instant",
            "Elemental",
        ),
        (
            "brigids_command",
            "Brigid's Command",
            "brigid_s_command",
            "{1}{G}{W}",
            "Sorcery",
            "Kithkin",
        ),
    ] {
        let card = registry.get(id).expect("Command registered");
        assert_eq!(card.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        let face = card.primary_face();
        assert_eq!(face.face_id.as_str(), face_id);
        assert_eq!(face.mana_cost.to_string(), mana);
        assert_eq!(
            face.types.iter().map(String::as_str).collect::<Vec<_>>(),
            ["Kindred", card_type, subtype]
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
            assert!(mode.targeting.is_some(), "{id} mode {index} is targeted");
        }
    }
}

#[test]
fn issue_445_token_modes_bind_the_recipient_to_one_player_target() {
    let registry = tricerules_cards::registry::global();
    for (id, mode_index, token, count) in [
        ("ashlings_command", 3usize, "treasure", 2u32),
        ("brigids_command", 1usize, "kithkin_gw_1_1", 1u32),
    ] {
        let modal = registry
            .get(id)
            .unwrap()
            .primary_face()
            .modal_spell
            .clone()
            .unwrap();
        let mode = &modal.modes[mode_index];
        let [SpellEffectKind::CreateTokens {
            token: actual_token,
            count: actual_count,
            who,
            tapped: false,
            sacrifice_timing: None,
        }] = mode.effects.as_slice()
        else {
            panic!(
                "{id} token mode is one untapped CreateTokens: {:?}",
                mode.effects
            );
        };
        assert_eq!(actual_token, token);
        assert_eq!(*actual_count, count.into());
        assert_eq!(*who, targeted_recipient());
        let groups = &mode.targeting.as_ref().unwrap().groups;
        assert_eq!(groups.len(), 1);
        assert_eq!((groups[0].min, groups[0].max), (1, 1));
        assert_eq!(groups[0].effect_indices, vec![0]);
    }

    let ashling = registry
        .get("ashlings_command")
        .unwrap()
        .primary_face()
        .modal_spell
        .clone()
        .unwrap();
    assert!(matches!(
        ashling.modes[2].effects.as_slice(),
        [SpellEffectKind::DamageAll {
            players: MassPlayerSet::TargetedPlayer {
                group_index: 0,
                kind: TargetKind::AnyPlayer
            },
            ..
        }]
    ));
}
