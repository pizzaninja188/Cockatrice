use crate::helpers::*;
use tricerules_proto::ruled::v1::LogMessage;

fn ability_log(batch: &RuledEventBatch) -> &LogMessage {
    batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::Log(log)) if log.ability_presentation.is_some() => Some(log),
            _ => None,
        })
        .expect("ability log with structured presentation")
}

#[test]
fn ability_logs_cover_activated_mana_sacrifice_and_targeted_abilities() {
    for (card_id, name, targets, sacrificed, mana) in [
        ("evolving_wilds", "Evolving Wilds", vec![], true, false),
        ("llanowar_elves", "Llanowar Elves", vec![], false, true),
        (
            "prodigal_sorcerer",
            "Prodigal Sorcerer",
            target_player(1),
            false,
            false,
        ),
        ("treasure", "Treasure", vec![], true, true),
    ] {
        let mut engine = GameEngine::new(207_001, &[0, 1], 20, None, true).unwrap();
        advance_to_main1_from_game_start(&mut engine);
        let source = inject_permanent_on_battlefield(&mut engine, 0, card_id);
        let batch = engine
            .apply_command(0, &activate_ability(source, 0, targets.clone()))
            .unwrap();
        let log = ability_log(&batch);
        let presentation = log.ability_presentation.as_ref().unwrap();
        let ability = presentation.ability.as_ref().unwrap();
        assert_eq!(ability.card_id, card_id);
        assert!(presentation
            .prefix
            .starts_with(&format!("P0 activates {name}")));
        assert_eq!(
            log.text,
            format!(
                "{}{}{}",
                presentation.prefix, ability.fallback_text, presentation.suffix
            )
        );
        if card_id != "treasure" {
            assert_eq!(ability.external_card_name, name);
            assert_eq!(ability.oracle_line_indices, vec![1]);
        }
        if !targets.is_empty() {
            assert!(!presentation.suffix.is_empty(), "retain target description");
        }
        if sacrificed {
            assert!(!engine.state.players[0].battlefield.contains(&source));
            assert!(
                presentation.prefix.contains("sacrific"),
                "retain cost receipt"
            );
        }
        if mana {
            assert!(engine.state.stack.is_empty());
        } else {
            let stack = batch
                .events
                .iter()
                .find_map(|event| match &event.ev {
                    Some(Ev::StackPushed(stack)) => Some(stack),
                    _ => None,
                })
                .unwrap();
            assert_eq!(stack.primary_presentation.as_ref(), Some(ability));
        }
    }
}

#[test]
fn ability_logs_preserve_optional_targeted_trigger_presentation_through_decline() {
    let mut engine = GameEngine::new(207_002, &[0, 1], 20, None, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "gravedigger");
    grant_pool(&mut engine, 0);
    engine
        .apply_command(
            0,
            &cast_spell(hand_index_for_card(&engine, 0, "gravedigger"), vec![]),
        )
        .unwrap();
    engine.apply_command(0, &pass()).unwrap();
    let batch = engine.apply_command(1, &pass()).unwrap();
    let ability = ability_log(&batch)
        .ability_presentation
        .as_ref()
        .unwrap()
        .ability
        .clone();
    assert_eq!(ability.as_ref().unwrap().external_card_name, "Gravedigger");
    assert_eq!(ability.as_ref().unwrap().oracle_line_indices, vec![1]);
    let prompt = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::TriggerNeedsTarget(prompt)) => Some(prompt),
            _ => None,
        })
        .unwrap();
    assert_eq!(prompt.ability_presentation, ability);
    let decline = RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: true,
            ..Default::default()
        })),
    };
    assert!(
        engine.apply_command(1, &decline).is_err(),
        "opponent cannot decline"
    );
    let declined = engine.apply_command(0, &decline).unwrap();
    let log = ability_log(&declined);
    assert!(log.text.starts_with("P0 declines optional trigger: "));
    assert_eq!(log.ability_presentation.as_ref().unwrap().ability, ability);
}
