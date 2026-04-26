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
    EXPECT_EQ(8, ruledPhaseLabelToCockatricePhase("end_combat"));
    EXPECT_EQ(9, ruledPhaseLabelToCockatricePhase("main2"));
    EXPECT_EQ(10, ruledPhaseLabelToCockatricePhase("end_step"));
    EXPECT_EQ(10, ruledPhaseLabelToCockatricePhase("cleanup"));
}

TEST(RuledUtilsTest, UnknownPhaseMapsToMinusOne)
{
    EXPECT_EQ(-1, ruledPhaseLabelToCockatricePhase("unknown_phase"));
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
