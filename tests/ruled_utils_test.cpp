#include "gtest/gtest.h"

#include "game/ruled_utils.h"

TEST(RuledUtilsTest, MapsKnownPhaseLabels)
{
    EXPECT_EQ(0, ruledPhaseLabelToCockatricePhase("untap"));
    EXPECT_EQ(1, ruledPhaseLabelToCockatricePhase("upkeep"));
    EXPECT_EQ(2, ruledPhaseLabelToCockatricePhase("draw"));
    EXPECT_EQ(3, ruledPhaseLabelToCockatricePhase("main1"));
    EXPECT_EQ(4, ruledPhaseLabelToCockatricePhase("begin_combat"));
    EXPECT_EQ(5, ruledPhaseLabelToCockatricePhase("declare_attackers"));
    EXPECT_EQ(6, ruledPhaseLabelToCockatricePhase("declare_blockers"));
    EXPECT_EQ(7, ruledPhaseLabelToCockatricePhase("combat_damage"));
    // CR 510.4: first-strike substep shares the combat damage toolbar slot; the prompt
    // widget differentiates via `first_strike_step_pending` on the per-player view.
    EXPECT_EQ(7, ruledPhaseLabelToCockatricePhase("first_strike_damage"));
    EXPECT_EQ(8, ruledPhaseLabelToCockatricePhase("end_combat"));
    EXPECT_EQ(9, ruledPhaseLabelToCockatricePhase("main2"));
    EXPECT_EQ(10, ruledPhaseLabelToCockatricePhase("end_step"));
    EXPECT_EQ(10, ruledPhaseLabelToCockatricePhase("cleanup"));
}

TEST(RuledUtilsTest, UnknownPhaseMapsToMinusOne)
{
    EXPECT_EQ(-1, ruledPhaseLabelToCockatricePhase("unknown_phase"));
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
