#include "mocks.h"
#include "test_card_database_path_provider.h"

#include "gtest/gtest.h"
#include <QBuffer>
#include <QFile>
#include <QTemporaryDir>
#include <libcockatrice/card/database/parser/cockatrice_xml_4.h>
#include <libcockatrice/interfaces/noop_card_preference_provider.h>
#include <libcockatrice/interfaces/noop_card_set_priority_controller.h>

namespace
{

TEST(CardDatabaseTest, PreservesOracleFacesThroughXmlRoundTrip)
{
    QByteArray source = R"(<cockatrice_carddatabase version="4"><cards><card>
      <name>Fire // Ice</name><text>Combined display text</text>
      <ruled-oracle><face card-name="Fire // Ice" face-name="Fire"><text>First face</text></face>
      <face card-name="Fire // Ice" face-name="Ice"><text>Second face</text></face></ruled-oracle>
      </card></cards></cockatrice_carddatabase>)";
    QBuffer input(&source);
    ASSERT_TRUE(input.open(QIODevice::ReadOnly));
    NoopCardPreferenceProvider preferences;
    NoopCardSetPriorityController priorities;
    CockatriceXml4Parser parser(&preferences, &priorities);
    CardNameMap cards;
    QObject::connect(&parser, &ICardDatabaseParser::addCard, [&cards](CardInfoPtr card) {
        cards.insert(card->getName(), card->clone());
        card->ruled().addFace("Fire // Ice", "Fire", "Changed after cloning");
    });
    parser.parseFile(input);
    ASSERT_EQ(cards.size(), 1);
    EXPECT_EQ(cards.value("Fire // Ice")->getText(), QStringLiteral("Combined display text"));
    QTemporaryDir directory;
    const QString path = directory.filePath("cards.xml");
    ASSERT_TRUE(parser.saveToFile({}, {}, cards, path));
    QFile output(path);
    ASSERT_TRUE(output.open(QIODevice::ReadOnly));
    const QByteArray saved = output.readAll();
    EXPECT_TRUE(saved.contains("face-name=\"Fire\""));
    EXPECT_TRUE(saved.contains("face-name=\"Ice\""));
    EXPECT_TRUE(saved.contains("First face"));
    EXPECT_TRUE(saved.contains("Second face"));
    output.seek(0);
    parser.parseFile(output);
    EXPECT_EQ(cards.value("Fire // Ice")
                  ->ruled()
                  .compatibleFaceText("Fire // Ice", "Ice", RuledOracleText::textSha256("Second face")),
              QStringLiteral("Second face"));
}

TEST(CardDatabaseTest, LoadXml)
{
    CardDatabase *db = new CardDatabase(nullptr, new NoopCardPreferenceProvider(), new TestCardDatabasePathProvider(),
                                        new NoopCardSetPriorityController());

    // ensure the card database is empty at start
    ASSERT_EQ(0, db->getCardList().size()) << "Cards not empty at start";
    ASSERT_EQ(0, db->getSetList().size()) << "Sets not empty at start";
    ASSERT_EQ(0, db->query()->getAllMainCardTypes().size()) << "Types not empty at start";
    ASSERT_EQ(NotLoaded, db->getLoadStatus()) << "Incorrect status at start";

    // load dummy cards and test result
    db->loadCardDatabases();
    ASSERT_EQ(16, db->getCardList().size()) << "Wrong card count after load";
    ASSERT_EQ(6, db->getSetList().size()) << "Wrong sets count after load";
    ASSERT_EQ(4, db->query()->getAllMainCardTypes().size()) << "Wrong types count after load";
    ASSERT_EQ(Ok, db->getLoadStatus()) << "Wrong status after load";

    // ensure the card database is empty after clear()
    db->clear();
    ASSERT_EQ(0, db->getCardList().size()) << "Cards not empty after clear";
    ASSERT_EQ(0, db->getSetList().size()) << "Sets not empty after clear";
    ASSERT_EQ(0, db->query()->getAllMainCardTypes().size()) << "Types not empty after clear";
    ASSERT_EQ(NotLoaded, db->getLoadStatus()) << "Incorrect status after clear";
}
} // namespace

int main(int argc, char **argv)
{
    ::testing::InitGoogleTest(&argc, argv);
    return RUN_ALL_TESTS();
}
