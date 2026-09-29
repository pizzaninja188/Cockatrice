//! Small reviewed scenario families, not a rules DSL. Expectations never come from card effects.
use super::{semantic::*, *};
use serde::Deserialize;
use std::collections::BTreeSet;
use tricerules_core::Zone;

#[derive(Deserialize)]
#[serde(tag = "family", rename_all = "snake_case", deny_unknown_fields)]
enum Row {
    Draw {
        card: String,
        mana: [u32; 6],
        surface: Surface,
        #[serde(default)]
        mode_index: Option<u32>,
        #[serde(default)]
        mode_id: Option<String>,
        recipient: usize,
        count: usize,
        food: usize,
    },
    Destroy {
        card: String,
        mana: [u32; 6],
        target: String,
        illegal_target: String,
    },
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Surface {
    Spell,
    Etb,
    Mode,
}

fn mana([w, u, b, r, g, c]: [u32; 6]) -> ManaGift {
    ManaGift { w, u, b, r, g, c }
}

pub(crate) fn run_rows(text: &str, mut engine: impl FnMut() -> GameEngine) -> BTreeSet<String> {
    let rows: Vec<Row> = serde_json::from_str(text).expect("valid, explicit scenario rows");
    assert!(!rows.is_empty(), "empty scenario selection");
    let mut exercised = BTreeSet::new();
    for row in rows {
        match row {
            Row::Draw {
                card,
                mana: gift,
                surface,
                mode_index,
                mode_id,
                recipient,
                count,
                food,
            } => {
                let surface = match surface {
                    Surface::Mode => DrawSurface::Mode {
                        index: mode_index.expect("mode index"),
                        id: mode_id.as_deref().expect("mode identity"),
                    },
                    Surface::Spell | Surface::Etb => {
                        assert!(
                            mode_index.is_none() && mode_id.is_none(),
                            "mode fields on nonmodal row"
                        );
                        if matches!(surface, Surface::Spell) {
                            DrawSurface::Spell
                        } else {
                            DrawSurface::Etb
                        }
                    }
                };
                exercise_draw_in(
                    engine(),
                    DrawCase {
                        card: &card,
                        mana: mana(gift),
                        surface,
                        recipient,
                        count,
                        food,
                    },
                )
                .require_exercised();
                exercised.insert(card);
            }
            Row::Destroy {
                card,
                mana: gift,
                target,
                illegal_target,
            } => {
                let mut e = engine();
                let source = inject_card_into_hand(&mut e, 0, &card);
                let victim = inject_permanent_on_battlefield(&mut e, 1, &target);
                let illegal = inject_permanent_on_battlefield(&mut e, 1, &illegal_target);
                let bystander = inject_permanent_on_battlefield(&mut e, 0, &target);
                give_mana(&mut e, 0, mana(gift));
                let slot = hand_index_for_card(&e, 0, &card);
                let before = format!("{:?}", e.state);
                assert!(
                    e.apply_command(0, &cast_spell(slot, target_object(illegal)))
                        .is_err(),
                    "illegal target accepted"
                );
                assert_eq!(
                    format!("{:?}", e.state),
                    before,
                    "rejected cast changed state"
                );
                accepted(&mut e, 0, &cast_spell(slot, target_object(victim)));
                assert_object(&e, source, &card, 0, 0, Zone::Stack, 1);
                complete(&mut e, 16, |_| None).require_exercised();
                assert_object(&e, source, &card, 0, 0, Zone::Graveyard, 2);
                assert_object(&e, victim, &target, 1, 1, Zone::Graveyard, 1);
                assert_object(&e, illegal, &illegal_target, 1, 1, Zone::Battlefield, 0);
                assert_object(&e, bystander, &target, 0, 0, Zone::Battlefield, 0);
                assert_eq!(
                    e.state.players.iter().map(|p| p.life).collect::<Vec<_>>(),
                    vec![20, 20]
                );
                assert_main_priority(&e, 0);
                exercised.insert(card);
            }
        }
    }
    exercised
}
