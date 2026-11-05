#include <QTextStream>
#include <gtest/gtest.h>
#include <libcockatrice/deck_list/deck_list.h>
#include <libcockatrice/deck_list/tree/deck_list_card_node.h>

static int mainboardSize(const DeckList &deck)
{
    int count = 0;
    for (const auto *card : deck.getCardNodes({"main"})) {
        count += card->getNumber();
    }
    return count;
}

TEST(DeckListCommanders, PlainTextPreservesDeclarationsWithoutAddingThemToTheMainboard)
{
    DeckList original(QStringLiteral("<cockatrice_deck version=\"1\"><zone name=\"main\"><card number=\"99\" "
                                     "name=\"Island\"/></zone></cockatrice_deck>"));
    original.setCommanders({CardRef{"Kami of the Crescent Moon", {}}});
    QString text = original.writeToString_Plain();
    QTextStream input(&text);
    DeckList parsed;
    ASSERT_TRUE(parsed.loadFromStream_Plain(input, false, [](const QString &name) { return name; }));
    ASSERT_EQ(parsed.getCommanders().size(), 1);
    EXPECT_EQ(parsed.getCommanders()[0].name, "Kami of the Crescent Moon");
    EXPECT_EQ(mainboardSize(parsed), 99);
    EXPECT_EQ(parsed.getSideboardSize(), 0);
}

TEST(DeckListCommanders, PlainTextUnderstandsCommanderSectionsAndPerCardPrefixes)
{
    for (QString text : {QStringLiteral("Commander\n1 Alpha\n1 Beta\n\nDeck\n98 Forest\n\nSideboard\n1 Island\n"),
                         QStringLiteral("CM: 1 Alpha\nCM: 1 Beta\n98 Forest\nSB: 1 Island\n")}) {
        QTextStream input(&text);
        DeckList parsed;
        ASSERT_TRUE(parsed.loadFromStream_Plain(input, false, [](const QString &name) { return name; }));
        ASSERT_EQ(parsed.getCommanders().size(), 2);
        EXPECT_EQ(parsed.getCommanders()[0].name, "Alpha");
        EXPECT_EQ(parsed.getCommanders()[1].name, "Beta");
        EXPECT_EQ(mainboardSize(parsed), 98);
        EXPECT_EQ(parsed.getSideboardSize(), 1);
    }
}

TEST(DeckListCommanders, PlainTextReplacesExplicitDeclarationsAndPreservesOtherMetadata)
{
    DeckList deck;
    deck.setCommanders({CardRef{"Old commander", {}}});
    deck.setBannerCard(CardRef{"Banner", "printing"});
    QString text = "Commander\n1 New commander\n\nDeck\n99 Forest\n";
    QTextStream input(&text);
    ASSERT_TRUE(deck.loadFromStream_Plain(input, true, [](const QString &name) { return name; }));
    ASSERT_EQ(deck.getCommanders().size(), 1);
    EXPECT_EQ(deck.getCommanders()[0].name, "New commander");
    EXPECT_EQ(deck.getBannerCard().name, "Banner");
    QString oversized = "CM: 10001 Commander\n";
    QTextStream invalid(&oversized);
    EXPECT_FALSE(deck.loadFromStream_Plain(invalid, false, [](const QString &name) { return name; }));
}

TEST(DeckListCommanders, NativeDeckKeepsDeclarationsSeparateFromTheMainboard)
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
    EXPECT_TRUE(restored.getCardList({DECK_ZONE_MAIN}).isEmpty());
    EXPECT_FALSE(restored.isBlankDeck());
}

TEST(DeckListCommanders, LegacyCustomZoneNamedCommanderRemainsSeparate)
{
    const DeckList deck(QStringLiteral(
        "<cockatrice_deck><commanders><commander>Kami of the Crescent Moon</commander></commanders>"
        "<zone name=\"main\"><zone name=\"commander\"><card number=\"1\" name=\"Island\"/></zone></zone>"
        "</cockatrice_deck>"));
    const DeckList restored(deck.writeToString_Native());
    EXPECT_EQ(restored.getCardList({DECK_ZONE_MAIN}), QStringList{"Island"});
    EXPECT_EQ(restored.getCardList({DECK_ZONE_COMMANDER}), QStringList{"Kami of the Crescent Moon"});
}

int main(int argc, char **argv)
{
    ::testing::InitGoogleTest(&argc, argv);
    return RUN_ALL_TESTS();
}
