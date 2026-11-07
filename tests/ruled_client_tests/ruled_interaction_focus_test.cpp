#include "game/ruled/ruled_interaction_focus.h"
#include "game/ruled/ruled_interaction_selection.h"

#include <gtest/gtest.h>

TEST(RuledInteractionSelection, PaymentManaSourcesBelongToTheActingSeat)
{
    RuledClientState state(nullptr);
    state.payment.begin();
    RuledAbilityEntry mana;
    mana.activatable = true;
    mana.manaAbility = true;
    // Public metadata describes both seats' untapped lands; it is not local permission.
    state.activatedAbilitiesByOid.insert(10, {mana});
    state.activatedAbilitiesByOid.insert(20, {mana});
    const auto eligibility = [&](quint32 oid, int controller, int actor) {
        return ruledPaymentCandidateEligibility(state, oid, true, controller, actor);
    };
    using E = RuledTargetClickEligibility;
    EXPECT_EQ(eligibility(10, 17, 17), E::Legal);
    EXPECT_EQ(eligibility(20, 29, 17), E::Illegal);
    EXPECT_EQ(eligibility(10, 17, 29), E::Illegal);
    EXPECT_EQ(eligibility(20, 29, 29), E::Legal);
    // A control change follows the current controller, not the printed card's owner.
    EXPECT_EQ(eligibility(20, 17, 17), E::Legal);
    EXPECT_EQ(eligibility(10, 17, -1), E::Illegal);
    state.activatedAbilitiesByOid[10][0]->activatable = false;
    EXPECT_EQ(eligibility(10, 17, 17), E::Illegal);
    state.payment.clear();
    EXPECT_EQ(eligibility(20, 29, 29), E::NotTargeting);
}

TEST(RuledInteractionSelection, PaymentContributionsKeepTheirExactPreviewAndGeneration)
{
    RuledClientState state(nullptr);
    state.payment.begin();
    state.battlefieldGenerationByOid.insert(30, 4);
    const auto request = state.payment.request({});
    ruled::v1::PaymentPreview preview;
    preview.set_transaction_id(request.transaction_id());
    preview.set_revision(request.revision());
    preview.set_valid(true);
    auto *candidate = preview.add_candidates();
    candidate->mutable_object()->set_object_id(30);
    candidate->mutable_object()->set_zone_change_generation(4);
    candidate->add_options(ruled::v1::OBJECT_PAYMENT_KIND_GENERIC);
    ASSERT_TRUE(state.payment.apply(preview));
    using E = RuledTargetClickEligibility;
    EXPECT_EQ(ruledPaymentCandidateEligibility(state, 30, true, 17, 17), E::Legal);
    EXPECT_EQ(ruledPaymentCandidateEligibility(state, 30, false, 17, 17), E::Illegal);
    EXPECT_EQ(ruledPaymentCandidateEligibility(state, 31, true, 17, 17), E::Illegal);
    state.battlefieldGenerationByOid[30] = 5;
    EXPECT_EQ(ruledPaymentCandidateEligibility(state, 30, true, 17, 17), E::Illegal);
    state.battlefieldGenerationByOid[30] = 4;
    ASSERT_TRUE(state.payment.select(30, ruled::v1::OBJECT_PAYMENT_KIND_GENERIC));
    state.payment.view.clear_candidates();
    // A selected contribution remains selectable so the player can remove it.
    EXPECT_EQ(ruledPaymentCandidateEligibility(state, 30, true, 17, 17), E::Legal);
}

TEST(RuledInteractionFocus, CombatSelectionOnlyUsesTheActingSeatsCandidates)
{
    RuledClientState state(nullptr);
    state.currentCombatPhase = RuledClientState::RuledCombatPhase::DeclareAttackers;
    state.currentActivePlayerId = 2;
    state.selectableAttackerOids = {10};
    EXPECT_EQ(ruledCombatSelection(state, 2, 10), RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledCombatSelection(state, 2, 11), RuledTargetClickEligibility::Illegal);
    EXPECT_EQ(ruledCombatSelection(state, 3, 10), RuledTargetClickEligibility::NotTargeting);
    state.attackersSubmittedThisStep = true;
    EXPECT_EQ(ruledCombatSelection(state, 2, 10), RuledTargetClickEligibility::NotTargeting);
    state.currentCombatPhase = RuledClientState::RuledCombatPhase::DeclareBlockers;
    state.currentPriorityPlayerId = 3;
    state.legalBlockAttackerOidsByBlocker = {{20, {10}}, {21, {}}};
    EXPECT_EQ(ruledCombatSelection(state, 3, 20), RuledTargetClickEligibility::Legal);
    EXPECT_EQ(ruledCombatSelection(state, 3, 21), RuledTargetClickEligibility::Illegal);
    EXPECT_EQ(ruledCombatSelection(state, 2, 20), RuledTargetClickEligibility::NotTargeting);
}

TEST(RuledInteractionSelection, CostCandidatesRespectDomainGenerationAndCompetingSelections)
{
    PendingRuledSpellCast spell;
    spell.valid = true;
    spell.waitingForCost = true;
    RuledCostChoice first;
    first.costIndex = 0;
    first.zone = RuledCostChoiceZone::Battlefield;
    first.kind = RuledCostChoiceKind::Sacrifice;
    first.candidateIds = {20, 21};
    first.candidateGenerations = {{20, 3}, {21, 4}};
    auto second = first;
    second.costIndex = 1;
    spell.costChoices = {first, second};
    spell.nextCostChoice = 1;
    RuledPendingCostSelection selected;
    selected.costIndex = 0;
    selected.zone = first.zone;
    selected.selectedIds = {20};
    spell.costSelections = {selected};
    const auto eligible = [&](RuledCostChoiceZone zone, quint32 oid, quint64 generation) {
        return ruledPendingCostCandidateEligibility(spell, zone, oid, oid, generation);
    };
    EXPECT_EQ(eligible(first.zone, 21, 4), RuledTargetClickEligibility::Legal);
    EXPECT_EQ(eligible(first.zone, 20, 3), RuledTargetClickEligibility::Illegal);
    EXPECT_EQ(eligible(first.zone, 21, 5), RuledTargetClickEligibility::Illegal);
    EXPECT_EQ(eligible(RuledCostChoiceZone::Hand, 21, 4), RuledTargetClickEligibility::Illegal);
    spell.costChoices[1].kind = RuledCostChoiceKind::Tap;
    EXPECT_EQ(eligible(first.zone, 20, 3), RuledTargetClickEligibility::Legal);
    spell.waitingForCost = false;
    EXPECT_EQ(eligible(first.zone, 21, 4), RuledTargetClickEligibility::NotTargeting);
}

TEST(RuledInteractionFocus, StackIncludesExactTargetsSourceAndPlayer)
{
    RuledClientState state(nullptr);
    state.stackOidOrder = {100, 101};
    state.stackCopySourceByStackOid.insert(100, 101);
    state.battlefieldGenerationByOid = {{20, 3}, {30, 7}};
    state.stackSourceCapturedOidByStackOid.insert(100, 20);
    state.stackSourceGenerationByStackOid.insert(100, 3);
    ruled::v1::TargetRef target;
    target.set_object_id(30);
    target.set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    target.set_expected_zone_change_generation(7);
    state.stackTargetReceiptsByStackOid[100].append(target);
    target.set_kind(ruled::v1::TARGET_REF_KIND_PLAYER);
    target.set_object_id(2);
    target.clear_expected_zone_change_generation();
    state.stackTargetReceiptsByStackOid[100].append(target);
    const auto focus = ruledStackFocus(state, 100);
    EXPECT_TRUE(focus.active);
    EXPECT_EQ(focus.battlefield, (QSet<quint32>{20, 30}));
    EXPECT_EQ(focus.stack, (QSet<quint32>{100, 101}));
    EXPECT_EQ(focus.players, QSet<int>{2});
    EXPECT_FALSE(ruledStackFocus(state, 999).active);

    state.battlefieldGenerationByOid[20] = 4;
    state.battlefieldGenerationByOid[30] = 8;
    EXPECT_TRUE(ruledStackFocus(state, 100).battlefield.isEmpty());
    state.stackOidOrder.removeOne(101);
    EXPECT_EQ(ruledStackFocus(state, 100).stack, QSet<quint32>{100});
    state.stackOidOrder.clear();
    EXPECT_FALSE(ruledStackFocus(state, 100).active);
}

TEST(RuledInteractionFocus, BlockHoverUsesExactOfferedPairsOnlyForTheDeclaringSeat)
{
    RuledClientState state(nullptr);
    state.currentCombatPhase = RuledClientState::RuledCombatPhase::DeclareBlockers;
    state.currentPriorityPlayerId = 2;
    state.currentAttackerOids = {10, 11};
    state.legalBlockAttackerOidsByBlocker = {{20, {10}}, {21, {11}}};
    const auto attacker = ruledCombatFocus(state, 10, 2);
    EXPECT_TRUE(attacker.active);
    EXPECT_EQ(attacker.battlefield, (QSet<quint32>{10, 20}));
    EXPECT_EQ(ruledCombatFocus(state, 21, 2).battlefield, (QSet<quint32>{11, 21}));

    // An observer sees actual assignments, never the other seat's private legal-pair hints.
    state.committedBlocks.insert(21, 10);
    EXPECT_EQ(ruledCombatFocus(state, 10, 3).battlefield, (QSet<quint32>{10, 21}));
    state.blockersSubmittedThisStep = true;
    EXPECT_EQ(ruledCombatFocus(state, 10, 2).battlefield, (QSet<quint32>{10, 21}));
}

TEST(RuledInteractionFocus, AttackerHoverIncludesExactBattleOrPlayerDefenders)
{
    RuledClientState state(nullptr);
    state.currentCombatPhase = RuledClientState::RuledCombatPhase::DeclareAttackers;
    state.currentActivePlayerId = 1;
    state.currentPriorityPlayerId = 1;
    state.selectableAttackerOids = {10};
    state.battlefieldGenerationByOid = {{10, 2}, {40, 9}};
    ruled::v1::AttackAssignment edge;
    edge.set_attacker_object_id(10);
    edge.set_attacker_zone_change_generation(2);
    edge.mutable_defender()->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    edge.mutable_defender()->set_object_id(40);
    edge.set_defender_zone_change_generation(9);
    state.legalAttackAssignmentsByAttacker[10].append(edge);
    edge.mutable_defender()->set_kind(ruled::v1::TARGET_REF_KIND_PLAYER);
    edge.mutable_defender()->set_object_id(3);
    state.legalAttackAssignmentsByAttacker[10].append(edge);
    const auto focus = ruledCombatFocus(state, 10, 1);
    EXPECT_TRUE(focus.active);
    EXPECT_EQ(focus.battlefield, (QSet<quint32>{10, 40}));
    EXPECT_EQ(focus.players, QSet<int>{3});
    state.battlefieldGenerationByOid[40] = 10;
    EXPECT_FALSE(ruledCombatFocus(state, 10, 1).battlefield.contains(40));
}
