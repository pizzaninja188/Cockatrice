//! Registry identity for the "Regenerate target creature." instants (issue #523). Names, costs
//! and types are the printed Oracle values; the shared scenario module executes every card.
use crate::common;

use common::FaceExpectation;
use tricerules_cards::primitives::{EffectSubject, ObjectFilter, SpellEffectKind, TargetKind};

#[test]
fn issue_523_registers_regenerate_target_creature_instants() {
    for (id, name, mana_cost) in [
        ("death_ward", "Death Ward", "{W}"),
        ("regenerate", "Regenerate", "{1}{G}"),
        ("mending_touch", "Mending Touch", "{G}"),
    ] {
        let face = FaceExpectation {
            id,
            name,
            face_id: id,
            mana_cost,
            types: &["Instant"],
            keywords: &[],
            power_toughness: None,
        }
        .check();
        assert_eq!(face.spell_effect.len(), 1, "{id}");
        let SpellEffectKind::Regenerate {
            subject: EffectSubject::Chosen(filter),
        } = &face.spell_effect[0]
        else {
            panic!("{id}: the single effect regenerates the chosen creature");
        };
        assert_eq!(
            **filter,
            ObjectFilter {
                kind: TargetKind::Creature,
                ..Default::default()
            },
            "{id}: any creature, no controller or other restriction"
        );
        let groups = &face.targeting.as_ref().expect("targeting").groups;
        assert_eq!(groups.len(), 1, "{id}");
        assert_eq!((groups[0].min, groups[0].max), (1, 1), "{id}");
        assert_eq!(groups[0].effect_indices, [0], "{id}");
        assert!(face.spell_program.is_none(), "{id}");
    }
}
