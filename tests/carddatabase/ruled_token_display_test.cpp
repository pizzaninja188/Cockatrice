#include "game/ruled/ruled_token_display.h"
#include "test_card_database_path_provider.h"

#include <gtest/gtest.h>
#include <libcockatrice/card/database/card_database.h>
#include <libcockatrice/interfaces/noop_card_preference_provider.h>
#include <libcockatrice/interfaces/noop_card_set_priority_controller.h>
#include <memory>

TEST(RuledTokenDisplayTest, ResolvesSparseProwessTokenDespitePrintedCardNameCollision)
{
    auto db = std::make_unique<CardDatabase>(nullptr, new NoopCardPreferenceProvider(),
                                             new TestCardDatabasePathProvider(), new NoopCardSetPriorityController());
    db->loadCardDatabases();
    ASSERT_EQ(db->getLoadStatus(), Ok);
    ASSERT_TRUE(db->query()->getCardInfo(QStringLiteral("Goblin Wizard")));
    ASSERT_FALSE(db->query()->getCardInfo(QStringLiteral("Goblin Wizard Token")));
    ASSERT_TRUE(db->query()->getCardInfo(QStringLiteral("Goblin Wizard Token ")));

    const CardRef resolved = RuledTokenDisplay::resolve(
        db->query(), QStringLiteral("Goblin Wizard"), QStringLiteral("1/1"), QStringLiteral("r"), {},
        {QStringLiteral(
            "Prowess (Whenever you cast a noncreature spell, this creature gets +1/+1 until end of turn.)")});
    EXPECT_EQ(resolved.name, QStringLiteral("Goblin Wizard Token "));

    const CardRef wrongPt = RuledTokenDisplay::resolve(
        db->query(), QStringLiteral("Goblin Wizard"), QStringLiteral("2/2"), QStringLiteral("r"), {},
        {QStringLiteral(
            "Prowess (Whenever you cast a noncreature spell, this creature gets +1/+1 until end of turn.)")});
    EXPECT_TRUE(wrongPt.name.isEmpty()) << "never substitute a token with the wrong printed P/T";
}

TEST(RuledTokenDisplayTest, ResolvesNoncreatureTokensByPrintedAbilityText)
{
    auto db = std::make_unique<CardDatabase>(nullptr, new NoopCardPreferenceProvider(),
                                             new TestCardDatabasePathProvider(), new NoopCardSetPriorityController());
    db->loadCardDatabases();
    ASSERT_EQ(db->getLoadStatus(), Ok);

    const CardRef clue = RuledTokenDisplay::resolve(db->query(), QStringLiteral("Clue"), {}, {}, {},
                                                    {QStringLiteral("{2}, Sacrifice this token: Draw a card.")});
    EXPECT_EQ(clue.name, QStringLiteral("Clue Token"));

    const CardRef lander = RuledTokenDisplay::resolve(
        db->query(), QStringLiteral("Lander"), {}, {}, {},
        {QStringLiteral("{2}, {T}, Sacrifice this token: Search your library for a basic land card, put it onto the "
                        "battlefield tapped, then shuffle.")});
    EXPECT_EQ(lander.name, QStringLiteral("Lander Token"));

    const CardRef wrongAbility =
        RuledTokenDisplay::resolve(db->query(), QStringLiteral("Clue"), {}, {}, {},
                                   {QStringLiteral("{2}, Sacrifice this token: You gain 3 life.")});
    EXPECT_TRUE(wrongAbility.name.isEmpty()) << "never substitute a token with different printed text";
}

TEST(RuledTokenDisplayTest, StableEngineFallbackResolvesOnlyOneStructuralTokenCandidate)
{
    auto db = std::make_unique<CardDatabase>(nullptr, new NoopCardPreferenceProvider(),
                                             new TestCardDatabasePathProvider(), new NoopCardSetPriorityController());
    db->loadCardDatabases();
    ASSERT_EQ(db->getLoadStatus(), Ok);

    const CardRef map = RuledTokenDisplay::resolve(db->query(), QStringLiteral("Map"), {}, {}, {},
                                                   {QStringLiteral("Map — activated ability (activated_01)")});
    EXPECT_EQ(map.name, QStringLiteral("Map Token"));

    const CardRef ambiguous = RuledTokenDisplay::resolve(db->query(), QStringLiteral("Marker"), {}, {}, {},
                                                         {QStringLiteral("Marker — activated ability (activated_01)")});
    EXPECT_TRUE(ambiguous.name.isEmpty()) << "a stable fallback must not guess between display variants";
}
