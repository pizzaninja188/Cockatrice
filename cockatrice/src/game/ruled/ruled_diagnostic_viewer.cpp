#include "ruled_diagnostic_viewer.h"

#include "../../interface/widgets/replay/replay_timeline_widget.h"
#include "../../interface/widgets/tabs/tab_game.h"
#include "../../interface/widgets/tabs/tab_supervisor.h"
#include "../../interface/window_main.h"
#include "../abstract_game.h"
#include "../player/player.h"
#include "../prompt/game_prompt_widget.h"
#include "ruled_client_diagnostics.h"
#include "ruled_client_state.h"

#include <QApplication>
#include <QCommandLineParser>
#include <QDockWidget>
#include <QEventLoop>
#include <QFileDialog>
#include <QGraphicsView>
#include <QInputDialog>
#include <QJsonArray>
#include <QJsonDocument>
#include <QLabel>
#include <QListWidget>
#include <QMessageBox>
#include <QPlainTextEdit>
#include <QPointer>
#include <QPushButton>
#include <QScreen>
#include <QScrollArea>
#include <QSignalBlocker>
#include <QSplitter>
#include <QStackedWidget>
#include <QTabWidget>
#include <QTemporaryDir>
#include <QTimer>
#include <QVBoxLayout>
#include <libcockatrice/protocol/ruled_diagnostic_archive.h>
#include <libcockatrice/protocol/ruled_diagnostic_journal.h>
#include <libcockatrice/protocol/ruled_diagnostic_reader.h>
#include <libcockatrice/protocol/ruled_diagnostics.h>

bool RuledDiagnosticViewer::isPlayback(const QObject *game)
{
    return game && game->property("ruledDiagnosticPlayback").toBool();
}
void RuledDiagnosticViewer::open(MainWindow *window)
{
    const auto archive = QFileDialog::getOpenFileName(window, tr("Open Bug Report"), {},
                                                      tr("Bug report ZIP (*.zip);;Capture manifest (manifest.json)"));
    if (archive.isEmpty())
        return;
    QString error;
    openPath(window, archive, -1, &error);
    if (!error.isEmpty())
        QMessageBox::warning(window, tr("Cannot open report"), error);
}
RuledDiagnosticViewer *
RuledDiagnosticViewer::openPath(MainWindow *window, const QString &archive, int requestedSeat, QString *failure)
{
    const auto fail = [failure](const QString &reason) -> RuledDiagnosticViewer * {
        *failure = reason;
        return nullptr;
    };
    auto temporary = std::make_shared<QTemporaryDir>();
    QString directory = QFileInfo(archive).isDir() ? archive : QFileInfo(archive).absolutePath(), error;
    if (archive.endsWith(".zip", Qt::CaseInsensitive)) {
        if (!temporary->isValid() || !RuledDiagnosticArchive::unpack(archive, temporary->path(), &error)) {
            return fail(error);
        }
        directory = temporary->path();
    }
    auto capture = std::make_shared<RuledDiagnosticReader>();
    if (!capture->load(directory, &error))
        return fail(error);
    int seat = capture->manifest.value("local_player_id").toInt(-1);
    const bool server = capture->manifest.value("source") == "server";
    if (server) {
        QSet<int> seats;
        for (const auto &row : capture->rows)
            if (row.value("kind") == "recipient_event")
                seats.insert(row.value("context").toObject().value("recipient_player_id").toInt(-1));
        auto ids = seats.values();
        std::sort(ids.begin(), ids.end());
        QStringList labels;
        for (const auto id : ids)
            labels.append(QString::number(id));
        if (labels.isEmpty())
            return fail(tr("No recipient events were captured."));
        if (requestedSeat >= 0) {
            if (!seats.contains(requestedSeat))
                return fail(tr("The requested seat has no recorded events."));
            seat = requestedSeat;
        } else {
            bool accepted = false;
            const auto selected = QInputDialog::getItem(window, tr("Recorded seat"), tr("Replay recipient player ID"),
                                                        labels, 0, false, &accepted);
            if (!accepted)
                return nullptr;
            seat = selected.toInt();
        }
    }
    auto replay = std::make_unique<GameReplay>();
    replay->mutable_game_info()->set_game_id(capture->manifest.value("game_id").toVariant().toInt());
    replay->mutable_game_info()->set_ruled_game(true);
    replay->mutable_game_info()->set_description("Recorded bug report");
    QList<quint64> sequences;
    for (const auto &row : capture->rows) {
        const auto kind = row.value("kind").toString();
        const bool info = kind == "client_game_info" || kind == "server_game_info";
        const bool received = server
                                  ? kind == "recipient_event" &&
                                        row.value("context").toObject().value("recipient_player_id").toInt(-1) == seat
                                  : kind == "received_game_events";
        if (!info && !received)
            continue;
        auto message = capture->message(row, &error);
        if (!message)
            return fail(error);
        if (info) {
            if (const auto *gameInfo = dynamic_cast<ServerInfo_Game *>(message.get()))
                replay->mutable_game_info()->CopyFrom(*gameInfo);
        } else if (const auto *event = dynamic_cast<GameEventContainer *>(message.get())) {
            replay->add_event_list()->CopyFrom(*event);
            sequences.append(row.value("sequence").toString().toULongLong());
        }
    }
    if (sequences.isEmpty())
        return fail(tr("No game events were captured."));
    replay->mutable_game_info()->set_started(false);
    auto *supervisor = window->getTabSupervisor();
    supervisor->openReplay(replay.release());
    auto *tab = qobject_cast<TabGame *>(supervisor->currentWidget());
    if (!tab)
        return fail(tr("Could not create the playback tab."));
    auto *game = tab->getGame();
    game->setProperty("ruledDiagnosticPlayback", true);
    game->getPlayerManager()->localPlayerId = seat;
    // Recipient ID controls private dispatch; no network client exists in a Replay game.
    game->getPlayerManager()->localPlayerIsSpectator =
        server ? seat < 0 : capture->manifest.value("spectator").toBool(seat < 0);
    game->getGameMetaInfo()->setSpectatorsOmniscient(false);
    auto *viewer = new RuledDiagnosticViewer(tab, capture, sequences);
    connect(viewer, &QObject::destroyed, [temporary] {}); // Keep extracted evidence alive with the tab.
    return viewer;
}

void RuledDiagnosticViewer::addCommandLineOptions(QCommandLineParser &parser)
{
    parser.addOptions({{"open-ruled-report", tr("Open a local ruled report or capture"), "path"},
                       {"report-seat", tr("Recipient player ID for a server capture"), "id"},
                       {"export-playback", tr("Export current-client playback comparison and exit"), "json"}});
}
bool RuledDiagnosticViewer::isBatchPlayback()
{
    return qApp->property("ruledDiagnosticBatchPlayback").toBool();
}
void RuledDiagnosticViewer::prepareBatchPlayback(const QCommandLineParser &parser)
{
    if (!parser.isSet("export-playback"))
        return;
    auto profile = std::make_shared<QTemporaryDir>();
    if (!profile->isValid() || !parser.isSet("open-ruled-report") || !parser.isSet("report-seat")) {
        qFatal("Batch playback requires --open-ruled-report, --report-seat, and a writable temporary directory");
    }
    qApp->setProperty("ruledDiagnosticBatchPlayback", true);
    qApp->setProperty("ruledDiagnosticProfile", profile->path());
    connect(qApp, &QObject::destroyed, [profile] {});
}
void RuledDiagnosticViewer::openFromCommandLine(MainWindow *window, const QCommandLineParser &parser)
{
    if (!parser.isSet("open-ruled-report"))
        return;
    const QString path = parser.value("open-ruled-report"), output = parser.value("export-playback");
    const int seat = parser.isSet("report-seat") ? parser.value("report-seat").toInt() : -1;
    QTimer::singleShot(0, window, [window, path, output, seat] {
        QString error;
        auto *viewer = openPath(window, path, seat, &error);
        if (!viewer) {
            qWarning() << "Report playback failed:" << error;
            if (!output.isEmpty())
                QCoreApplication::exit(1);
            else
                QMessageBox::warning(window, tr("Cannot open report"), error);
            return;
        }
        if (output.isEmpty())
            return;
        QTimer::singleShot(0, viewer, [viewer, output] {
            auto *evidenceDock = viewer->tab->findChild<QDockWidget *>("ruledDiagnosticViewer");
            if (!viewer->tab->replayDock->isFloating() || !evidenceDock->isAncestorOf(viewer->events)) {
                qWarning() << "Playback bar must start detached; event list must remain in the evidence panel";
                QCoreApplication::exit(1);
                return;
            }
            // Model a maintainer moving the evidence out beside the timeline. Recorded state
            // transitions and long prompts must preserve that arrangement.
            evidenceDock->setFloating(true);
            evidenceDock->resize(560, 480);
            // Let initial application/tab layout requests settle before measuring user geometry.
            QEventLoop layoutReady;
            QTimer::singleShot(100, &layoutReady, &QEventLoop::quit);
            layoutReady.exec();
            const auto windowSize = viewer->tab->window()->size();
            const auto timelineGeometry = viewer->tab->replayDock->geometry();
            const auto evidenceGeometry = evidenceDock->geometry();
            const auto stableLayout = [&] {
                QCoreApplication::processEvents();
                if (!viewer->tab->replayDock->isFloating() || !evidenceDock->isFloating() ||
                    viewer->tab->window()->size() != windowSize ||
                    viewer->tab->replayDock->geometry() != timelineGeometry ||
                    evidenceDock->geometry() != evidenceGeometry) {
                    QString error;
                    RuledDiagnosticJournal::writeJson(
                        output + ".layout-error.json",
                        {{"main_before", QJsonArray{windowSize.width(), windowSize.height()}},
                         {"main_after", QJsonArray{viewer->tab->window()->width(), viewer->tab->window()->height()}},
                         {"timeline_before", QJsonArray{timelineGeometry.x(), timelineGeometry.y(),
                                                        timelineGeometry.width(), timelineGeometry.height()}},
                         {"timeline_after",
                          QJsonArray{viewer->tab->replayDock->x(), viewer->tab->replayDock->y(),
                                     viewer->tab->replayDock->width(), viewer->tab->replayDock->height()}},
                         {"evidence_before", QJsonArray{evidenceGeometry.x(), evidenceGeometry.y(),
                                                        evidenceGeometry.width(), evidenceGeometry.height()}},
                         {"evidence_after", QJsonArray{evidenceDock->x(), evidenceDock->y(), evidenceDock->width(),
                                                       evidenceDock->height()}}},
                        &error);
                    qWarning() << "Playback changed the window or detached panel geometry during a seek" << windowSize
                               << viewer->tab->window()->size() << timelineGeometry
                               << viewer->tab->replayDock->geometry() << evidenceGeometry << evidenceDock->geometry();
                    QCoreApplication::exit(1);
                    return false;
                }
                return true;
            };
            viewer->tab->gamePromptWidget->setPromptText(QStringLiteral("Long recorded prompt\n").repeated(150));
            if (!stableLayout())
                return;
            // Exercise separate user seeks, including the deferred player destruction between clicks.
            // A single synchronous rewind/forward pair can conceal stale signal receivers.
            for (int pass = 0; pass < 2; ++pass) {
                QList<QPair<QPointer<Player>, QPointer<PlayerGraphicsItem>>> retiredGraphics;
                for (auto *player : viewer->tab->getGame()->getPlayerManager()->getPlayers())
                    retiredGraphics.append({player, player->getGraphicsItem()});
                viewer->seek(viewer->reader->rows.first().value("sequence").toString().toULongLong());
                QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
                if (!stableLayout())
                    return;
                for (const auto &[player, graphics] : retiredGraphics) {
                    if (!player && graphics) {
                        qWarning() << "Playback rewind retained a retired player's graphics and signal receivers";
                        QCoreApplication::exit(1);
                        return;
                    }
                }
                viewer->seek(viewer->reader->rows.last().value("sequence").toString().toULongLong());
                QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
                if (!stableLayout())
                    return;
            }
            QString error;
            const bool ok = RuledDiagnosticJournal::writeJson(output, viewer->comparison, &error);
            if (!ok)
                qWarning() << "Playback export failed:" << error;
            QCoreApplication::exit(ok ? 0 : 1);
        });
    });
}
RuledDiagnosticViewer::RuledDiagnosticViewer(TabGame *tab,
                                             std::shared_ptr<RuledDiagnosticReader> reader,
                                             const QList<quint64> &sequences)
    : QObject(tab), tab(tab), timeline(tab->findChild<ReplayTimelineWidget *>()), reader(std::move(reader)),
      eventSequences(sequences)
{
    qApp->installEventFilter(this);
    // A recorded recipient is local for private-event dispatch, but has no live deck/ready UI.
    // Recreating that UI on rewind changes the stacked widget's minimum size and leaks old deck views.
    auto *handler = tab->getGame()->getGameEventHandler();
    disconnect(handler, &GameEventHandler::localPlayerReadyStateChanged, tab,
               &TabGame::processLocalPlayerReadyStateChanged);
    disconnect(handler, &GameEventHandler::localPlayerSideboardLocked, tab,
               &TabGame::processLocalPlayerSideboardLocked);
    disconnect(handler, &GameEventHandler::localPlayerDeckSelected, tab, &TabGame::processLocalPlayerDeckSelect);
    tab->mainWidget->setCurrentWidget(tab->gamePlayAreaWidget);
    auto *dock = new QDockWidget(tr("Bug report — recorded evidence / current playback"), tab);
    dock->setObjectName("ruledDiagnosticViewer");
    auto *panel = new QWidget(dock);
    auto *layout = new QVBoxLayout(panel);
    auto *notice = new QLabel(
        tr("Read-only recipient playback. The board runs received events through the current client. Recorded clicks "
           "and staged payments appear in the evidence pane; playback does not recreate mouse timing."),
        panel);
    notice->setWordWrap(true);
    layout->addWidget(notice);
    if (!this->reader->warnings.isEmpty()) {
        auto *warning = new QLabel(this->reader->warnings.join('\n'), panel);
        warning->setWordWrap(true);
        layout->addWidget(warning);
    }
    // Detach the playback bar; keep event navigation beside the evidence editors.
    tab->replayDock->setWindowTitle(tr("Timeline"));
    tab->replayDock->setFloating(true);
    const auto available = tab->screen()->availableGeometry();
    tab->replayDock->resize(qMin(720, available.width() - 48), 100);
    tab->replayDock->move(available.topLeft() + QPoint(24, 24));
    tab->replayDock->show();

    auto *split = new QSplitter(panel);
    events = new QListWidget(split);
    auto *pages = new QTabWidget(split);
    recordedText = new QPlainTextEdit(pages);
    recordedText->setReadOnly(true);
    currentText = new QPlainTextEdit(pages);
    currentText->setReadOnly(true);
    pages->addTab(recordedText, tr("Recorded UI / event"));
    pages->addTab(currentText, tr("Current playback / differences"));
    QFile report(this->reader->directory + "/report.md");
    if (report.open(QIODevice::ReadOnly) && report.size() < 1024 * 1024) {
        auto *text = new QPlainTextEdit(pages);
        text->setReadOnly(true);
        text->setPlainText(QString::fromUtf8(report.readAll()));
        pages->addTab(text, tr("Report"));
    }
    QPixmap screenshot(this->reader->directory + "/screenshots/game.png");
    if (!screenshot.isNull()) {
        auto *viewport = new QScrollArea(pages);
        auto *image = new QLabel(viewport);
        image->setPixmap(screenshot.scaled(720, 500, Qt::KeepAspectRatio, Qt::SmoothTransformation));
        viewport->setWidget(image);
        pages->addTab(viewport, tr("Recorded screenshot"));
    }
    layout->addWidget(split, 1);
    auto *promptViewport = new QScrollArea(panel);
    promptViewport->setWidgetResizable(true);
    promptViewport->setMinimumHeight(100);
    promptViewport->setMaximumHeight(160);
    tab->gamePromptWidget = new GamePromptWidget(promptViewport);
    tab->gamePromptWidget->setEnabled(false);
    promptViewport->setWidget(tab->gamePromptWidget);
    layout->addWidget(promptViewport);
    auto *exportButton = new QPushButton(tr("Export recorded / playback comparison…"), panel);
    layout->addWidget(exportButton);
    connect(exportButton, &QPushButton::clicked, this, [this] {
        const auto path = QFileDialog::getSaveFileName(this->tab, tr("Export playback comparison"),
                                                       "playback-comparison.json", tr("JSON (*.json)"));
        if (path.isEmpty())
            return;
        QString error;
        if (!RuledDiagnosticJournal::writeJson(path, comparison, &error))
            QMessageBox::warning(this->tab, tr("Export failed"), error);
    });
    for (const auto &row : this->reader->rows) {
        events->addItem(row.value("sequence").toString() + "  " + row.value("kind").toString() + "  " +
                        row.value("correlation_id").toString());
    }
    connect(events, &QListWidget::currentRowChanged, this, [this](int index) {
        if (index >= 0 && index < this->reader->rows.size())
            seek(this->reader->rows[index].value("sequence").toString().toULongLong());
    });
    dock->setWidget(panel);
    tab->addDockWidget(Qt::BottomDockWidgetArea, dock);
    if (timeline) {
        QList<int> positions;
        for (int i = 0; i < eventSequences.size(); ++i)
            positions.append(i * 1000);
        timeline->setTimeline(positions);
        timeline->maxTime += 200;
        connect(timeline, &ReplayTimelineWidget::rewound, this, [this] { reset(); });
        connect(timeline, &ReplayTimelineWidget::processNextEvent, this, [this](EventProcessingOptions) {
            if (!seeking && timeline->getCurrentEvent() < eventSequences.size()) {
                const auto index = timeline->getCurrentEvent();
                const auto sequence = index + 1 < eventSequences.size()
                                          ? eventSequences[index + 1] - 1
                                          : this->reader->rows.last().value("sequence").toString().toULongLong();
                QTimer::singleShot(0, this, [this, sequence] { display(sequence); });
            }
        });
    }
    QTimer::singleShot(0, this, [this] {
        if (!this->reader->rows.isEmpty())
            events->setCurrentRow(this->reader->rows.size() - 1);
    });
}
void RuledDiagnosticViewer::reset()
{
    auto *game = tab->getGame();
    game->getGameEventHandler()->clearRuledSessionState(RuledSessionResetScope::All);
    const auto players = game->getPlayerManager()->getPlayers();
    for (auto it = players.begin(); it != players.end(); ++it) {
        it.value()->clear();
        game->getPlayerManager()->removePlayer(it.key());
        // removePlayer detaches the graphics from the scene, but only queues the Player's deletion.
        // Retire the detached graphics now: their ruled-state connections must not outlive the player.
        delete it.value()->getGraphicsItem();
    }
    game->getPlayerManager()->spectators.clear();
    game->getGameMetaInfo()->setStarted(false);
    game->getGameState()->setGameStateKnown(false);
    game->getGameState()->setActivePlayer(-1);
    game->getGameState()->setPriorityPlayer(-1);
}
void RuledDiagnosticViewer::seek(quint64 sequence)
{
    if (timeline) {
        timeline->stopReplay();
        const auto next = std::upper_bound(eventSequences.begin(), eventSequences.end(), sequence);
        const auto count = std::distance(eventSequences.begin(), next);
        seeking = true;
        timeline->skipToTime(count == 0 ? 0 : int((count - 1) * 1000 + 200), false);
        seeking = false;
    }
    display(sequence);
}
void RuledDiagnosticViewer::display(quint64 sequence)
{
    auto *game = tab->getGame();
    tab->gamePromptWidget->setLocalPlayerHasPriority(game->getGameState()->getPriorityPlayer() ==
                                                     game->getPlayerManager()->getLocalPlayerId());
    tab->gamePromptWidget->setActivePhase(game->getGameState()->getCurrentPhase());
    tab->refreshRuledPromptState();
    QString error;
    const auto recorded = reader->state("client", sequence, &error);
    QJsonObject row;
    for (int i = 0; i < reader->rows.size(); ++i) {
        if (reader->rows[i].value("sequence").toString().toULongLong() == sequence) {
            row = reader->rows[i];
            QSignalBlocker blocker(events);
            events->setCurrentRow(i);
            break;
        }
    }
    const auto current = game->getGameEventHandler()->diagnostics()->currentState();
    comparison = {{"format_version", 1},
                  {"privacy", "recipient_only"},
                  {"source_capture_id", reader->manifest.value("capture_id")},
                  {"recorded_sequence", QString::number(sequence)},
                  {"playback_build", RuledDiagnostics::buildInfo()},
                  {"recorded_client", error.isEmpty() ? QJsonValue(recorded) : QJsonValue()},
                  {"recorded_state_unavailable", error},
                  {"current_playback_client", current},
                  {"differences", error.isEmpty() ? RuledDiagnostics::differences(recorded, current) : QJsonArray()}};
    recordedText->setPlainText(QString::fromUtf8(
        QJsonDocument(QJsonObject{{"record", row}, {"recorded_client", recorded}, {"unavailable_reason", error}})
            .toJson(QJsonDocument::Indented)));
    currentText->setPlainText(QString::fromUtf8(QJsonDocument(comparison).toJson(QJsonDocument::Indented)));
}
bool RuledDiagnosticViewer::eventFilter(QObject *object, QEvent *event)
{
    if (event->type() != QEvent::MouseButtonPress && event->type() != QEvent::MouseButtonRelease &&
        event->type() != QEvent::MouseButtonDblClick && event->type() != QEvent::KeyPress)
        return false;
    auto *widget = qobject_cast<QWidget *>(object);
    if (!widget || !tab->isAncestorOf(widget))
        return false;
    for (auto *parent = widget; parent && parent != tab; parent = parent->parentWidget())
        if (qobject_cast<QGraphicsView *>(parent))
            return true;
    return false;
}
