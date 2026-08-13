#include "gtest/gtest.h"

#include "game/ruled_utils.h"

TEST(RuledUtilsTest, MapsKnownPhases)
{
    EXPECT_EQ(0, ruledPhaseToCockatricePhase(ruled::v1::PHASE_ID_UNTAP));
    EXPECT_EQ(1, ruledPhaseToCockatricePhase(ruled::v1::PHASE_ID_UPKEEP));
    EXPECT_EQ(2, ruledPhaseToCockatricePhase(ruled::v1::PHASE_ID_DRAW));
    EXPECT_EQ(3, ruledPhaseToCockatricePhase(ruled::v1::PHASE_ID_MAIN1));
    EXPECT_EQ(4, ruledPhaseToCockatricePhase(ruled::v1::PHASE_ID_BEGIN_COMBAT));
    EXPECT_EQ(5, ruledPhaseToCockatricePhase(ruled::v1::PHASE_ID_DECLARE_ATTACKERS));
    EXPECT_EQ(6, ruledPhaseToCockatricePhase(ruled::v1::PHASE_ID_DECLARE_BLOCKERS));
    EXPECT_EQ(7, ruledPhaseToCockatricePhase(ruled::v1::PHASE_ID_COMBAT_DAMAGE));
    // CR 510.4: first-strike substep shares the combat damage toolbar slot; the prompt
    // widget differentiates via `first_strike_step_pending` on the per-player view.
    EXPECT_EQ(7, ruledPhaseToCockatricePhase(ruled::v1::PHASE_ID_FIRST_STRIKE_DAMAGE));
    EXPECT_EQ(8, ruledPhaseToCockatricePhase(ruled::v1::PHASE_ID_END_COMBAT));
    EXPECT_EQ(9, ruledPhaseToCockatricePhase(ruled::v1::PHASE_ID_MAIN2));
    EXPECT_EQ(10, ruledPhaseToCockatricePhase(ruled::v1::PHASE_ID_END_STEP));
    EXPECT_EQ(10, ruledPhaseToCockatricePhase(ruled::v1::PHASE_ID_CLEANUP));
}

TEST(RuledUtilsTest, PhasesWithoutAToolbarSlotMapToMinusOne)
{
    // The opening procedure and the assign-combat-damage pause have no toolbar slot, and
    // neither does an unset or unknown value.
    EXPECT_EQ(-1, ruledPhaseToCockatricePhase(ruled::v1::PHASE_ID_OPENING_CHOOSE_FIRST));
    EXPECT_EQ(-1, ruledPhaseToCockatricePhase(ruled::v1::PHASE_ID_OPENING_MULLIGAN));
    EXPECT_EQ(-1, ruledPhaseToCockatricePhase(ruled::v1::PHASE_ID_ASSIGN_COMBAT_DAMAGE));
    EXPECT_EQ(-1, ruledPhaseToCockatricePhase(ruled::v1::PHASE_ID_UNSPECIFIED));
    EXPECT_EQ(-1, ruledPhaseToCockatricePhase(static_cast<ruled::v1::PhaseId>(99)));
}

TEST(RuledUtilsTest, PrivateChoiceKindsAreTheConcealedZoneOnes)
{
    // Private: the candidates live in a zone the other players cannot see, so the relay must
    // strip them from everyone but the deciding player.
    EXPECT_TRUE(isPrivateChoiceKind(ruled::v1::CHOICE_KIND_HAND_CARDS));
    EXPECT_TRUE(isPrivateChoiceKind(ruled::v1::CHOICE_KIND_LIBRARY_SEARCH));
    EXPECT_TRUE(isPrivateChoiceKind(ruled::v1::CHOICE_KIND_OPPONENT_HAND));
    // Public: already revealed to the table, or on the battlefield.
    EXPECT_FALSE(isPrivateChoiceKind(ruled::v1::CHOICE_KIND_REVEALED));
    EXPECT_FALSE(isPrivateChoiceKind(ruled::v1::CHOICE_KIND_TARGET_OBJECTS));
    EXPECT_FALSE(isPrivateChoiceKind(ruled::v1::CHOICE_KIND_LEGEND_KEEP));
    EXPECT_FALSE(isPrivateChoiceKind(ruled::v1::CHOICE_KIND_REPLACEMENT_EFFECT));
    EXPECT_FALSE(isPrivateChoiceKind(ruled::v1::CHOICE_KIND_COPY_SOURCE));
    EXPECT_FALSE(isPrivateChoiceKind(ruled::v1::CHOICE_KIND_MANA_PAYMENT));
    // A kind this build does not know about is treated as concealing something.
    EXPECT_TRUE(isPrivateChoiceKind(static_cast<ruled::v1::ChoiceKind>(99)));
}

TEST(RuledUtilsTest, ManaPoolCounterNameValidation)
{
    EXPECT_TRUE(isRuledModeManaPoolCounterName("w"));
    EXPECT_TRUE(isRuledModeManaPoolCounterName("U"));
    EXPECT_TRUE(isRuledModeManaPoolCounterName(" c "));
    EXPECT_FALSE(isRuledModeManaPoolCounterName("life"));
    EXPECT_FALSE(isRuledModeManaPoolCounterName("zz"));
}

int main(int argc, char **argv)
{
    ::testing::InitGoogleTest(&argc, argv);
    return RUN_ALL_TESTS();
}
