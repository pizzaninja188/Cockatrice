#include "../../oracle/src/oracleimporter.h"

#include <QFile>
#include <QTemporaryDir>
#include <gtest/gtest.h>
#include <libcockatrice/card/database/parser/cockatrice_xml_4.h>
#include <libcockatrice/card/set/card_set.h>
#include <libcockatrice/interfaces/noop_card_preference_provider.h>
#include <libcockatrice/interfaces/noop_card_set_priority_controller.h>

TEST(RuledOracleImportTest, SavesExactFacesInDatabaseWithoutCompanionFile)
{
    NoopCardPreferenceProvider preferences;
    NoopCardSetPriorityController priorities;
    auto set = CardSet::newInstance(&priorities, "TST", "Test", "expansion", QDate(2026, 1, 1));
    OracleImporter importer;
    const QList<QVariant> cards{
        QVariantMap{{"name", "Forest"},
                    {"text", "{T}: Add {G}."},
                    {"layout", "normal"},
                    {"types", QStringList{"Land"}},
                    {"number", "1"}},
        QVariantMap{{"name", "Fire // Ice"},
                    {"faceName", "Fire"},
                    {"text", "Fire text"},
                    {"layout", "split"},
                    {"types", QStringList{"Instant"}},
                    {"number", "2"}},
        QVariantMap{{"name", "Fire // Ice"},
                    {"faceName", "Ice"},
                    {"text", "Ice text"},
                    {"layout", "split"},
                    {"types", QStringList{"Instant"}},
                    {"number", "2"}},
        QVariantMap{{"name", "Beanstalk Giant // Fertile Footsteps"},
                    {"faceName", "Beanstalk Giant"},
                    {"text", "Giant text"},
                    {"layout", "adventure"},
                    {"types", QStringList{"Creature"}},
                    {"number", "3"}},
        QVariantMap{{"name", "Beanstalk Giant // Fertile Footsteps"},
                    {"faceName", "Fertile Footsteps"},
                    {"text", "Adventure text"},
                    {"layout", "adventure"},
                    {"types", QStringList{"Sorcery"}},
                    {"number", "3"}},
        QVariantMap{{"name", "Brutal Cathar // Moonrage Brute"},
                    {"faceName", "Moonrage Brute"},
                    {"text", "Back face text"},
                    {"layout", "transform"},
                    {"side", "b"},
                    {"types", QStringList{"Creature"}},
                    {"number", "4"}},
        QVariantMap{{"name", "Bala Ged Recovery // Bala Ged Sanctuary"},
                    {"faceName", "Bala Ged Sanctuary"},
                    {"text", "Land face text"},
                    {"layout", "modal_dfc"},
                    {"side", "b"},
                    {"types", QStringList{"Land"}},
                    {"number", "5"}},
    };
    importer.importCardsFromSet(set, cards);
    QTemporaryDir directory;
    const QString path = directory.filePath("cards.xml");
    ASSERT_TRUE(importer.saveToFile(path, "https://example.test/source", "snapshot"));
    EXPECT_FALSE(QFile::exists(directory.filePath("cards.ruled-oracle.json")));
    QFile input(path);
    ASSERT_TRUE(input.open(QIODevice::ReadOnly));
    CockatriceXml4Parser parser(&preferences, &priorities);
    CardNameMap loaded;
    QObject::connect(&parser, &ICardDatabaseParser::addCard,
                     [&loaded](CardInfoPtr card) { loaded.insert(card->getName(), card); });
    parser.parseFile(input);
    for (const auto &record : cards) {
        const auto data = record.toMap();
        const QString cardName = data.value("name").toString();
        const QString faceName = data.value("faceName", cardName).toString();
        const QString text = data.value("text").toString();
        const auto card = loaded.value(data.contains("side") ? faceName : cardName);
        ASSERT_TRUE(card) << faceName.toStdString();
        EXPECT_EQ(card->ruled().compatibleFaceText(cardName, faceName, RuledOracleText::textSha256(text)), text)
            << faceName.toStdString();
    }
    EXPECT_EQ(loaded.value("Fire // Ice")->getText(), QStringLiteral("Fire text\n\n---\n\nIce text"));
}
