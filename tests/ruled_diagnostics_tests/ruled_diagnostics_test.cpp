#include <QDateTime>
#include <QDir>
#include <QJsonArray>
#include <QJsonDocument>
#include <QTemporaryDir>
#include <gtest/gtest.h>
#include <libcockatrice/protocol/pb/event_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/game_event_container.pb.h>
#include <libcockatrice/protocol/pb/ruled_diagnostics.pb.h>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <libcockatrice/protocol/ruled_diagnostic_archive.h>
#include <libcockatrice/protocol/ruled_diagnostic_journal.h>
#include <libcockatrice/protocol/ruled_diagnostic_reader.h>
#include <libcockatrice/protocol/ruled_diagnostics.h>

TEST(RuledDiagnostics, DecodesLosslessIntegersAndAbsentFields)
{
    ruled::v1::SessionStart start;
    start.set_seed(18446744073709551615ULL);
    start.add_player_ids(17);
    const auto decoded = RuledDiagnostics::decode(start);
    EXPECT_EQ(decoded.value("seed").toString(), QStringLiteral("18446744073709551615"));
    EXPECT_EQ(decoded.value("player_ids").toArray().at(0).toInt(), 17);
    EXPECT_TRUE(decoded.value("player_decks").isArray());
    EXPECT_TRUE(decoded.value("player_decks").toArray().isEmpty());
}

TEST(RuledDiagnostics, DecodesExtensionsEmbeddedPayloadAndEnumNames)
{
    ruled::v1::RuledEventBatch batch;
    batch.add_events()->mutable_phase_changed()->set_phase_id(ruled::v1::PHASE_ID_MAIN1);
    GameEventContainer container;
    container.add_event_list()->MutableExtension(Event_RuledPayload::ext)->set_payload(batch.SerializeAsString());
    const auto decoded = RuledDiagnostics::decode(container);
    const auto event = decoded.value("event_list").toArray().at(0).toObject();
    const auto wrapper = event.value("Event_RuledPayload.ext").toObject();
    const auto payload = wrapper.value("payload").toObject();
    EXPECT_EQ(payload.value("events")
                  .toArray()
                  .at(0)
                  .toObject()
                  .value("phase_changed")
                  .toObject()
                  .value("phase_id")
                  .toString(),
              QStringLiteral("PHASE_ID_MAIN1"));
    EXPECT_TRUE(payload.value("payment_preview").isNull());
}

TEST(RuledDiagnostics, DifferencesDistinguishMissingNullAndEmpty)
{
    const QJsonObject before{{"a/b", QJsonValue::Null}, {"items", QJsonArray{1, 2}}};
    const QJsonObject after{{"a/b", QJsonArray{}}, {"items", QJsonArray{1, 3}}, {"new", true}};
    const auto changes = RuledDiagnostics::differences(before, after);
    ASSERT_EQ(changes.size(), 3);
    EXPECT_EQ(changes.at(0).toObject().value("path").toString(), QStringLiteral("/a~1b"));
    EXPECT_EQ(changes.at(1).toObject().value("path").toString(), QStringLiteral("/items/1"));
    EXPECT_EQ(changes.at(2).toObject().value("operation").toString(), QStringLiteral("add"));
}

TEST(RuledDiagnostics, JournalPersistsRequestBeforeOutcomeAndExactRawRecord)
{
    QTemporaryDir root;
    RuledDiagnosticJournal journal(root.path(), "relay", "server_only");
    ASSERT_TRUE(journal.isHealthy()) << journal.error().toStdString();
    ruled::v1::IpcEnvelope envelope;
    envelope.mutable_player_command()->set_player_id(17);
    envelope.mutable_player_command()->set_ruled_command("bad protobuf");
    EXPECT_EQ(journal.record("engine_request", envelope, "client-1"), 1U);
    QFile timeline(journal.directory() + "/timeline.jsonl");
    ASSERT_TRUE(timeline.open(QIODevice::ReadOnly));
    const auto row = QJsonDocument::fromJson(timeline.readLine()).object();
    EXPECT_EQ(row.value("sequence").toString(), QStringLiteral("1"));
    EXPECT_EQ(row.value("correlation_id").toString(), QStringLiteral("client-1"));
    QFile raw(journal.directory() + "/" + row.value("raw_ref").toString());
    ASSERT_TRUE(raw.open(QIODevice::ReadOnly));
    const auto bytes = raw.readAll();
    ruled::diagnostics::Record record;
    ASSERT_TRUE(record.ParseFromArray(bytes.constData(), bytes.size()));
    EXPECT_EQ(record.payload(), envelope.SerializeAsString());
    EXPECT_EQ(record.sequence(), 1U);
    QFile manifest(journal.directory() + "/manifest.json");
    ASSERT_TRUE(manifest.open(QIODevice::ReadOnly));
    EXPECT_FALSE(QJsonDocument::fromJson(manifest.readAll()).object().value("complete").toBool());
    journal.note("engine_transport_error", {{"reason", "connection lost"}}, "client-1");
    journal.finish();
}

TEST(RuledDiagnostics, StateSnapshotsHaveReadableDifferencesAndReportRetention)
{
    QTemporaryDir root;
    RuledDiagnosticJournal journal(root.path(), "client", "recipient_only");
    ASSERT_TRUE(journal.isHealthy());
    journal.state("ui", {{"prompt", "Choose targets"}, {"selected", QJsonArray{}}});
    journal.state("ui", {{"prompt", "Confirm payment"}, {"selected", QJsonArray{42}}});
    journal.markReport("report-1");
    QFile timeline(journal.directory() + "/timeline.jsonl");
    ASSERT_TRUE(timeline.open(QIODevice::ReadOnly));
    auto first = QJsonDocument::fromJson(timeline.readLine()).object();
    auto second = QJsonDocument::fromJson(timeline.readLine()).object();
    EXPECT_EQ(first.value("kind").toString(), QStringLiteral("state_snapshot"));
    EXPECT_EQ(second.value("kind").toString(), QStringLiteral("state_delta"));
    EXPECT_EQ(second.value("data").toObject().value("changes").toArray().size(), 2);
    QFile manifest(journal.directory() + "/manifest.json");
    ASSERT_TRUE(manifest.open(QIODevice::ReadOnly));
    EXPECT_TRUE(
        QJsonDocument::fromJson(manifest.readAll()).object().value("report_ids").toArray().contains("report-1"));
}

TEST(RuledDiagnostics, CaptureLimitIsExplicitAndDoesNotPretendToBeComplete)
{
    QTemporaryDir root;
    RuledDiagnosticJournal journal(root.path(), "client", "recipient_only", {}, 64);
    journal.note("large", {{"text", QString(256, 'x')}});
    EXPECT_FALSE(journal.isHealthy());
    EXPECT_FALSE(journal.error().isEmpty());
    journal.finish();
    QFile manifest(journal.directory() + "/manifest.json");
    ASSERT_TRUE(manifest.open(QIODevice::ReadOnly));
    const auto data = QJsonDocument::fromJson(manifest.readAll()).object();
    EXPECT_FALSE(data.value("complete").toBool());
    EXPECT_EQ(data.value("status").toString(), QStringLiteral("incomplete"));
}

TEST(RuledDiagnostics, CaptureQuotaIncludesLatestSnapshotStorage)
{
    QTemporaryDir root;
    RuledDiagnosticJournal journal(root.path(), "client", "recipient_only", {}, 80000);
    for (const auto letter : {'a', 'b', 'c'})
        journal.state("client", {{"text", QString(15000, QLatin1Char(letter))}});
    EXPECT_FALSE(journal.isHealthy());
    EXPECT_TRUE(journal.error().contains("limit"));
}

TEST(RuledDiagnostics, PruneRetainsActiveAndReportedCapturesAndRemovesExpiredPrefixes)
{
    QTemporaryDir root;
    const auto oldDate = QDateTime::currentDateTimeUtc().addDays(-9).toString(Qt::ISODateWithMs);
    RuledDiagnosticJournal active(root.path(), "server", "server_only");
    QString expiredPath, pinnedPath;
    {
        RuledDiagnosticJournal expired(root.path(), "server", "server_only");
        expired.note("unfinished_request", {});
        expiredPath = expired.directory();
    }
    {
        RuledDiagnosticJournal pinned(root.path(), "server", "server_only");
        pinned.markReport("local-report");
        pinned.finish();
        pinnedPath = pinned.directory();
    }
    QFile manifest(expiredPath + "/manifest.json");
    ASSERT_TRUE(manifest.open(QIODevice::ReadOnly));
    auto metadata = QJsonDocument::fromJson(manifest.readAll()).object();
    manifest.close();
    metadata.insert("updated_utc", oldDate);
    ASSERT_TRUE(RuledDiagnosticJournal::writeJson(manifest.fileName(), metadata));
    RuledDiagnosticJournal::prune(root.path(), 1);
    EXPECT_FALSE(QFileInfo::exists(expiredPath));
    EXPECT_TRUE(QFileInfo::exists(pinnedPath));
    EXPECT_TRUE(QFileInfo::exists(active.directory()));
}

TEST(RuledDiagnostics, ArchiveRoundTripAndRejectsCorruption)
{
    QTemporaryDir source, destination, archive;
    ASSERT_TRUE(QDir(source.path()).mkpath("raw"));
    QFile input(source.filePath("raw/one.pb"));
    ASSERT_TRUE(input.open(QIODevice::WriteOnly));
    input.write(QByteArray("a\0b", 3));
    input.close();
    QString error;
    const auto zip = archive.filePath("capture.zip");
    ASSERT_TRUE(RuledDiagnosticArchive::pack(source.path(), zip, &error)) << error.toStdString();
    ASSERT_TRUE(RuledDiagnosticArchive::unpack(zip, destination.path(), &error)) << error.toStdString();
    QFile output(destination.filePath("raw/one.pb"));
    ASSERT_TRUE(output.open(QIODevice::ReadOnly));
    EXPECT_EQ(output.readAll(), QByteArray("a\0b", 3));
    QTemporaryDir corruptDestination;
    QFile corrupt(zip);
    ASSERT_TRUE(corrupt.open(QIODevice::ReadWrite));
    corrupt.seek(30 + QByteArray("raw/one.pb").size());
    corrupt.write("x", 1);
    corrupt.close();
    EXPECT_FALSE(RuledDiagnosticArchive::unpack(zip, corruptDestination.path(), &error));
}

TEST(RuledDiagnostics, ReportExporterPreservesRecipientPrefixAndRejectsServerSource)
{
    QTemporaryDir root, output, extracted;
    RuledDiagnosticJournal client(root.path(), "client", "recipient_only");
    client.state("client", {{"own_hand", QJsonArray{"Mountain"}}});
    client.incomplete("test capture gap");
    const auto zip = output.filePath("report.zip");
    QString error;
    const QJsonObject report{{"report_id", "report-1"},
                             {"expected", "Choose a card"},
                             {"actual", "No options"},
                             {"notes", "Local evidence"}};
    ASSERT_TRUE(RuledDiagnosticArchive::exportReport(client.directory(), zip, report, {}, &error))
        << error.toStdString();
    ASSERT_TRUE(RuledDiagnosticArchive::unpack(zip, extracted.path(), &error));
    RuledDiagnosticReader reader;
    ASSERT_TRUE(reader.load(extracted.path(), &error)) << error.toStdString();
    EXPECT_FALSE(reader.manifest.value("complete").toBool());
    EXPECT_EQ(reader.manifest.value("privacy"), "recipient_only");
    QFile reportText(extracted.filePath("report.md"));
    ASSERT_TRUE(reportText.open(QIODevice::ReadOnly));
    EXPECT_TRUE(reportText.readAll().contains("No options"));
    EXPECT_FALSE(QFileInfo::exists(extracted.filePath("screenshots/game.png")));
    RuledDiagnosticJournal server(root.path(), "server", "server_only");
    EXPECT_FALSE(
        RuledDiagnosticArchive::exportReport(server.directory(), output.filePath("unsafe.zip"), report, {}, &error));
}

TEST(RuledDiagnostics, ReaderValidatesRawReadableAndReconstructsSnapshotDeltas)
{
    QTemporaryDir root;
    RuledDiagnosticJournal journal(root.path(), "client", "recipient_only");
    ruled::v1::SessionStart start;
    start.set_seed(std::numeric_limits<quint64>::max());
    journal.record("test", start);
    journal.state("client", {{"choice", "a"}, {"identity", QJsonObject{{"oid", 17}}}});
    const auto boundary = journal.sequence();
    journal.state("client", {{"choice", "b"}, {"identity", QJsonObject{{"oid", 29}}}});
    journal.finish();
    QString error;
    RuledDiagnosticReader reader;
    ASSERT_TRUE(reader.load(journal.directory(), &error)) << error.toStdString();
    EXPECT_EQ(reader.state("client", boundary, &error).value("choice").toString(), "a");
    EXPECT_EQ(reader.state("client", journal.sequence(), &error).value("identity").toObject().value("oid").toInt(), 29);
    QFile timeline(journal.directory() + "/timeline.jsonl");
    ASSERT_TRUE(timeline.open(QIODevice::ReadOnly));
    auto lines = timeline.readAll().split('\n');
    timeline.close();
    auto first = QJsonDocument::fromJson(lines[0]).object();
    first.insert("data", QJsonObject{{"seed", "wrong"}});
    lines[0] = QJsonDocument(first).toJson(QJsonDocument::Compact);
    ASSERT_TRUE(timeline.open(QIODevice::WriteOnly | QIODevice::Truncate));
    timeline.write(lines.join('\n'));
    timeline.close();
    EXPECT_FALSE(reader.load(journal.directory(), &error));
    EXPECT_TRUE(error.contains("readable"));
}
