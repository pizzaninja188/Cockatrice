//! Actual-card entry scenarios for five pinned Standard cards and exact printed tokens.
//! Scryfall Oracle and rulings checked 2026-09-22; no card-specific rulings.
//! CR 111.2–111.4 and 603.6a govern token identity and enters-the-battlefield triggers.

use super::helpers::*;
use tricerules_cards::{Color, Keyword};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    e
}

#[test]
fn issue_misc27_entry_triggers_create_exact_token_identities() {
    for (index, (card, token, count, types, stats, colors, keywords, source_keyword)) in [
        (
            "nimble_thopterist",
            "thopter_c_1_1_flying",
            1,
            &["Artifact", "Creature", "Thopter"][..],
            (1, 1),
            &[][..],
            &[Keyword::Flying][..],
            None,
        ),
        (
            "eager_glyphmage",
            "inkling_wb_1_1_flying",
            1,
            &["Creature", "Inkling"][..],
            (1, 1),
            &[Color::White, Color::Black][..],
            &[Keyword::Flying][..],
            None,
        ),
        (
            "mechanized_ninja_cavalry",
            "robot_c_1_1",
            1,
            &["Artifact", "Creature", "Robot"][..],
            (1, 1),
            &[][..],
            &[][..],
            None,
        ),
        (
            "oltec_cloud_guard",
            "gnome_c_1_1",
            1,
            &["Artifact", "Creature", "Gnome"][..],
            (1, 1),
            &[][..],
            &[][..],
            Some(Keyword::Flying),
        ),
        (
            "guarded_heir",
            "knight_w_3_3",
            2,
            &["Creature", "Knight"][..],
            (3, 3),
            &[Color::White][..],
            &[][..],
            Some(Keyword::Lifelink),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(827_001 + index as u64);
        inject_card_into_hand(&mut e, 0, card);
        grant_pool(&mut e, 0);
        let slot = hand_index_for_card(&e, 0, card);
        semantic::accepted(&mut e, 0, &cast_spell(slot, vec![]));
        assert!(
            battlefield_token_oids(&e, 0, token).is_empty(),
            "{card} has not entered yet"
        );
        resolve_entire_stack_two_player(&mut e);

        let source = *e.state.players[0]
            .battlefield
            .iter()
            .find(|&&id| e.state.objects[&id].card_id == card)
            .unwrap_or_else(|| panic!("missing {card} on battlefield"));
        if let Some(keyword) = source_keyword {
            assert!(e.effective_has_keyword(source, keyword), "{card}");
        }
        let tokens = battlefield_token_oids(&e, 0, token);
        assert_eq!(tokens.len(), count, "{card}");
        assert!(
            battlefield_token_oids(&e, 1, token).is_empty(),
            "{card} owns its tokens"
        );
        for id in tokens {
            assert!(e.state.objects[&id].is_token(), "{card}");
            let ch = e.characteristics(id).unwrap();
            assert_eq!(ch.types.as_slice(), types, "{card}");
            assert_eq!(
                (ch.power, ch.toughness),
                (Some(stats.0), Some(stats.1)),
                "{card}"
            );
            assert_eq!(ch.colors.as_slice(), colors, "{card}");
            assert_eq!(ch.keywords.as_slice(), keywords, "{card}");
        }
    }
}
