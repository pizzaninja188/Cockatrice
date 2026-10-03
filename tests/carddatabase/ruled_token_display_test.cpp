#include "game/ruled/ruled_token_display.h"
#include "test_card_database_path_provider.h"

#include <gtest/gtest.h>
#include <libcockatrice/card/database/card_database.h>
#include <libcockatrice/interfaces/noop_card_preference_provider.h>
#include <libcockatrice/interfaces/noop_card_set_priority_controller.h>
#include <memory>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <libcockatrice/protocol/pb/serverinfo_card.pb.h>

TEST(RuledTokenDisplayTest, QualifiedMyrRulesNameResolvesExactTokenArtwork)
{
    auto db = std::make_unique<CardDatabase>(nullptr, new NoopCardPreferenceProvider(),
        new TestCardDatabasePathProvider(), new NoopCardSetPriorityController());
    db->loadCardDatabases();
    ASSERT_EQ(db->getLoadStatus(), Ok);
    const CardRef qualified = RuledTokenDisplay::resolve(db->query(), "Myr Token", "1/1", "", {}, {});
    EXPECT_EQ(qualified.name, QStringLiteral("Myr Token"));
    const CardRef unqualified = RuledTokenDisplay::resolve(db->query(), "Myr", "1/1", "", {}, {});
    EXPECT_EQ(unqualified.name, QStringLiteral("Myr Token"));
    EXPECT_TRUE(RuledTokenDisplay::resolve(db->query(), "Myr Token", "2/2", "", {}, {}).name.isEmpty());
    EXPECT_TRUE(RuledTokenDisplay::resolve(db->query(), "Myr Token", "1/1", "r", {}, {}).name.isEmpty());
    ruled::v1::TokenIdentity identity;
    identity.set_name("Myr Token");
    identity.set_pt("1/1");
    identity.set_is_creature(true);
    for (const char *type : {"Artifact", "Creature", "Myr"}) identity.add_types(type);
    ServerInfo_Card physical;
    physical.set_id(47);
    RuledTokenDisplay::applyProposal(physical, identity, db->query());
    EXPECT_EQ(physical.id(), 47);
    EXPECT_EQ(physical.name(), "Myr Token");
    EXPECT_EQ(physical.pt(), "1/1");
    EXPECT_NE(physical.annotation().find("1/1 | Colorless | Artifact Creature Myr"), std::string::npos);
}

TEST(RuledTokenDisplayTest, SameNameGolemProposalsUseExactArtAndKeepCompleteMissingArtLabels)
{
    auto db = std::make_unique<CardDatabase>(nullptr, new NoopCardPreferenceProvider(),
        new TestCardDatabasePathProvider(), new NoopCardSetPriorityController());
    db->loadCardDatabases();
    ASSERT_EQ(db->getLoadStatus(), Ok);
    int index = 0;
    for (const char *keyword : {"Flying", "Vigilance", "Trample"}) {
        ruled::v1::TokenIdentity identity;
        identity.set_name("Golem");
        identity.set_pt("3/3");
        identity.set_is_creature(true);
        for (const char *type : {"Artifact", "Creature", "Golem"}) identity.add_types(type);
        identity.add_keywords(keyword);
        ServerInfo_Card popup;
        popup.set_id(index);
        RuledTokenDisplay::applyProposal(popup, identity, db->query());
        EXPECT_EQ(popup.name(), std::string("Golem Token") + std::string(index, ' '));
        EXPECT_EQ(popup.id(), index);
        EXPECT_EQ(popup.pt(), "3/3");
        EXPECT_NE(popup.annotation().find(keyword), std::string::npos);
        ServerInfo_Card missingArt;
        RuledTokenDisplay::applyProposal(missingArt, identity, nullptr);
        EXPECT_EQ(missingArt.name(), "Golem");
        EXPECT_EQ(missingArt.annotation(), popup.annotation());
        EXPECT_NE(missingArt.annotation().find("3/3 | Colorless | Artifact Creature Golem"), std::string::npos);
        ++index;
    }
    const auto missing = RuledTokenDisplay::resolve(db->query(), "Golem", "3/3", "", {"Lifelink"}, {});
    EXPECT_TRUE(missing.name.isEmpty());
}

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
