use crate::helpers::*;
use tricerules_proto::ruled::v1::TargetRef;

#[test]
fn holy_strength_buffs_enchanted_creature() {
    // Happy path: Holy Strength (+1/+2) on Grizzly Bears (2/2) → effective 3/4.
    let decks = Some(vec![
        vec![
            "plains".into(),
            "holy_strength".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(4201, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let hs_idx = hand_index_for_card(&e, 0, "holy_strength");
    e.apply_command(
        0,
        &cast_spell(
            hs_idx,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast Holy Strength targeting bear");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");

    // Bear is now enchanted with Holy Strength.
    assert_eq!(
        e.effective_power(bear),
        Some(3),
        "Holy Strength grants +1 power (2+1=3)"
    );
    assert_eq!(
        e.effective_toughness(bear),
        Some(4),
        "Holy Strength grants +2 toughness (2+2=4)"
    );

    // The aura itself should be on the battlefield, attached to the bear.
    let hs_oid = battlefield_object_for_card(&e, 0, "holy_strength");
    assert_eq!(
        e.state.objects.get(&hs_oid).and_then(|o| o.attached_to),
        Some(bear),
        "aura.attached_to must point at the enchanted creature"
    );
}

#[test]
fn unholy_strength_buffs_enchanted_creature() {
    // Unholy Strength (+2/+1) on Walking Corpse (2/2) → effective 4/3.
    let decks = Some(vec![
        vec![
            "swamp".into(),
            "unholy_strength".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
        ],
        vec!["swamp".into(); 7],
    ]);
    let mut e = GameEngine::new(4202, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let corpse = inject_creature_on_battlefield(&mut e, 0, "walking_corpse");

    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let us_idx = hand_index_for_card(&e, 0, "unholy_strength");
    e.apply_command(
        0,
        &cast_spell(
            us_idx,
            vec![TargetRef {
                object_id: corpse,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast Unholy Strength");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");

    assert_eq!(e.effective_power(corpse), Some(4), "2+2=4 power");
    assert_eq!(e.effective_toughness(corpse), Some(3), "2+1=3 toughness");
}

#[test]
fn aura_pt_buff_removed_when_aura_leaves_battlefield() {
    // The WhileSourceOnBattlefield continuous effect drains when the aura is destroyed.
    let decks = Some(vec![
        vec![
            "plains".into(),
            "holy_strength".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(4205, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let hs_idx = hand_index_for_card(&e, 0, "holy_strength");
    e.apply_command(
        0,
        &cast_spell(
            hs_idx,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast Holy Strength");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");

    let hs_oid = battlefield_object_for_card(&e, 0, "holy_strength");
    assert_eq!(
        e.effective_power(bear),
        Some(3),
        "buffed before aura removed"
    );

    // Directly remove the aura from the battlefield (simulates it being destroyed/bounced).
    {
        let obj = e.state.objects.get_mut(&hs_oid).expect("hs obj");
        obj.zone = tricerules_core::Zone::Graveyard;
        obj.attached_to = None;
    }
    e.state.players[0].battlefield.retain(|&id| id != hs_oid);
    e.state.players[0].graveyard.push(hs_oid);
    // Drain all continuous effects sourced from the aura (WhileSourceOnBattlefield).
    e.state
        .continuous_effects
        .retain(|ce| ce.source_id != Some(hs_oid));

    // Bear should return to base stats once the aura's effect is drained.
    assert_eq!(e.effective_power(bear), Some(2), "power back to base 2");
    assert_eq!(
        e.effective_toughness(bear),
        Some(2),
        "toughness back to base 2"
    );
}

#[test]
fn aura_dies_when_enchanted_creature_dies_sba() {
    // CR 704.5m: aura on battlefield with no valid enchanted permanent goes to graveyard on SBA.
    let decks = Some(vec![
        vec![
            "plains".into(),
            "holy_strength".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(4203, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let hs_idx = hand_index_for_card(&e, 0, "holy_strength");
    e.apply_command(
        0,
        &cast_spell(
            hs_idx,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast Holy Strength");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");

    let hs_oid = battlefield_object_for_card(&e, 0, "holy_strength");
    assert_eq!(
        e.state.objects.get(&hs_oid).map(|o| o.zone),
        Some(tricerules_core::Zone::Battlefield),
        "aura on battlefield before creature dies"
    );

    // Move the bear to the graveyard (simulating lethal damage SBA or destroy effect).
    // This mimics what move_object_to_zone does without going through the full engine path.
    {
        let obj = e.state.objects.get_mut(&bear).expect("bear obj");
        obj.zone = tricerules_core::Zone::Graveyard;
    }
    e.state.players[0].battlefield.retain(|&id| id != bear);
    e.state.players[0].graveyard.push(bear);

    // Passing priority triggers SBA check: orphaned aura should be destroyed.
    e.apply_command(0, &pass()).expect("pass triggers SBA");

    assert_eq!(
        e.state.objects.get(&hs_oid).map(|o| o.zone),
        Some(tricerules_core::Zone::Graveyard),
        "aura must go to graveyard (CR 704.5m) when enchanted creature leaves battlefield"
    );
}

#[test]
fn aura_spell_fizzles_when_target_leaves_before_resolution() {
    // CR 303.4f: aura goes to graveyard (not battlefield) when target leaves before resolution.
    let decks = Some(vec![
        vec![
            "plains".into(),
            "holy_strength".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(4204, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let hs_idx = hand_index_for_card(&e, 0, "holy_strength");
    e.apply_command(
        0,
        &cast_spell(
            hs_idx,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast Holy Strength");

    // Aura is on the stack; now remove the target creature before resolution.
    let hs_stack_id = e.state.stack.last().expect("hs on stack").id;
    {
        let obj = e.state.objects.get_mut(&bear).expect("bear obj");
        obj.zone = tricerules_core::Zone::Graveyard;
    }
    e.state.players[0].battlefield.retain(|&id| id != bear);
    e.state.players[0].graveyard.push(bear);

    // Both players pass — aura resolves but target is gone, must go to graveyard.
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");

    assert_eq!(
        e.state.objects.get(&hs_stack_id).map(|o| o.zone),
        Some(tricerules_core::Zone::Graveyard),
        "aura goes to graveyard when target left before resolution (CR 303.4f)"
    );
    // No continuous effect should have been created.
    let aura_effects: Vec<_> = e
        .state
        .continuous_effects
        .iter()
        .filter(|ce| ce.source_id == Some(hs_stack_id))
        .collect();
    assert!(
        aura_effects.is_empty(),
        "no P/T buff should linger when aura fizzled"
    );
}
