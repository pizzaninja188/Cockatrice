#include <gtest/gtest.h>
#include <libcockatrice/deck_list/deck_list.h>

TEST(DeckListCommanders, NativeMetadataKeepsDeclarationsSeparateFromTheMainboard)
{
    DeckList source;
    const QList<CardRef> commanders = {
        {QStringLiteral("Atraxa, Praetors' Voice"), QStringLiteral("print-a")},
        {QStringLiteral("Kraum, Ludevic's Opus"), QStringLiteral("print-b")},
    };
    source.setCommanders(commanders);
    source.setBannerCard(commanders.front());

    const DeckList restored(source.writeToString_Native());
    EXPECT_EQ(restored.getCommanders(), commanders);
    EXPECT_EQ(restored.getBannerCard(), commanders.front());
    EXPECT_TRUE(restored.getCardList().isEmpty());
    EXPECT_FALSE(restored.isBlankDeck());
}

int main(int argc, char **argv)
{
    ::testing::InitGoogleTest(&argc, argv);
    return RUN_ALL_TESTS();
}
