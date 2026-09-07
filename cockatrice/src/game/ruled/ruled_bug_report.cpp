#include "ruled_bug_report.h"

#include "../../interface/widgets/tabs/tab_game.h"
#include "../../interface/widgets/tabs/tab_supervisor.h"
#include "../../interface/window_main.h"
#include "../abstract_game.h"
#include "ruled_actions.h"
#include "ruled_client_diagnostics.h"
#include "ruled_diagnostic_viewer.h"

#include <QBuffer>
#include <QCheckBox>
#include <QComboBox>
#include <QDialog>
#include <QDialogButtonBox>
#include <QDirIterator>
#include <QFileDialog>
#include <QFormLayout>
#include <QJsonDocument>
#include <QLabel>
#include <QMenu>
#include <QMessageBox>
#include <QPlainTextEdit>
#include <QPointer>
#include <QTemporaryDir>
#include <QUuid>
#include <libcockatrice/protocol/ruled_diagnostic_archive.h>
#include <libcockatrice/protocol/ruled_diagnostic_journal.h>

namespace
{
void report(MainWindow *window)
{
    struct Candidate
    {
        QPointer<TabGame> tab;
        QPixmap screenshot;
    };
    QList<Candidate> candidates;
    auto *tabs = window->getTabSupervisor();
    int active = 0;
    for (int i = 0; i < tabs->count(); ++i) {
        auto *tab = qobject_cast<TabGame *>(tabs->widget(i));
        if (!tab || !RuledActions::isRuledGame(tab->getGame()))
            continue;
        auto *capture = tab->getGame()->getGameEventHandler()->diagnostics();
        capture->ensure();
        if (capture->directory().isEmpty() || !QFileInfo::exists(capture->directory() + "/manifest.json"))
            continue;
        if (tabs->currentWidget() == tab)
            active = candidates.size();
        capture->snapshot();
        candidates.append({tab, tab->grab()});
    }
    if (candidates.isEmpty()) {
        QMessageBox::information(window, QObject::tr("Report Bug"),
                                 QObject::tr("Open a ruled game with capture enabled to export a report."));
        return;
    }
    QDialog dialog(window);
    dialog.setWindowTitle(QObject::tr("Report Bug"));
    dialog.resize(640, 620);
    auto *layout = new QFormLayout(&dialog);
    auto *gameChoice = new QComboBox(&dialog);
    for (const auto &candidate : candidates)
        gameChoice->addItem(QString("%1 — %2")
                                .arg(candidate.tab->getGame()->getGameMetaInfo()->gameId())
                                .arg(candidate.tab->getGame()->getGameMetaInfo()->description()));
    gameChoice->setCurrentIndex(active);
    layout->addRow(QObject::tr("Game"), gameChoice);
    auto *expected = new QPlainTextEdit(&dialog), *actual = new QPlainTextEdit(&dialog),
         *notes = new QPlainTextEdit(&dialog);
    layout->addRow(QObject::tr("Expected behavior"), expected);
    layout->addRow(QObject::tr("Actual behavior"), actual);
    layout->addRow(QObject::tr("Notes / steps"), notes);
    auto *includeScreenshot = new QCheckBox(QObject::tr("Include game screenshot"), &dialog);
    includeScreenshot->setChecked(true);
    layout->addRow(includeScreenshot);
    auto *preview = new QLabel(&dialog);
    preview->setMaximumHeight(160);
    const auto refreshPreview = [=, &candidates] {
        preview->setPixmap(candidates[gameChoice->currentIndex()].screenshot.scaled(560, 160, Qt::KeepAspectRatio,
                                                                                    Qt::SmoothTransformation));
        preview->setVisible(includeScreenshot->isChecked());
    };
    QObject::connect(gameChoice, &QComboBox::currentIndexChanged, &dialog, refreshPreview);
    QObject::connect(includeScreenshot, &QCheckBox::toggled, &dialog, refreshPreview);
    refreshPreview();
    layout->addRow(preview);
    auto *privacy = new QLabel(
        QObject::tr("Exports this client's received game data and local UI state. It may include your private cards "
                    "and game chat. Save locally, review, then share with your maintainer. Nothing is uploaded."),
        &dialog);
    privacy->setWordWrap(true);
    layout->addRow(privacy);
    auto *buttons = new QDialogButtonBox(QDialogButtonBox::Save | QDialogButtonBox::Cancel, &dialog);
    QObject::connect(buttons, &QDialogButtonBox::accepted, &dialog, &QDialog::accept);
    QObject::connect(buttons, &QDialogButtonBox::rejected, &dialog, &QDialog::reject);
    layout->addRow(buttons);
    if (dialog.exec() != QDialog::Accepted)
        return;
    const auto selected = candidates[gameChoice->currentIndex()];
    if (!selected.tab)
        return;
    const auto reportId = QUuid::createUuid().toString(QUuid::WithoutBraces);
    const auto destination =
        QFileDialog::getSaveFileName(window, QObject::tr("Export Bug Report"), "ruled-report-" + reportId + ".zip",
                                     QObject::tr("ZIP archive (*.zip)"));
    if (destination.isEmpty())
        return;
    auto *capture = selected.tab->getGame()->getGameEventHandler()->diagnostics();
    const auto serverId = capture->markReport(reportId);
    QString error;
    QByteArray screenshot;
    QBuffer buffer(&screenshot);
    buffer.open(QIODevice::WriteOnly);
    bool okay = !includeScreenshot->isChecked() || selected.screenshot.save(&buffer, "PNG");
    const QJsonObject report{{"format_version", 1},
                             {"report_id", reportId},
                             {"server_capture_id", serverId},
                             {"expected", expected->toPlainText()},
                             {"actual", actual->toPlainText()},
                             {"notes", notes->toPlainText()},
                             {"capture_error", capture->error()}};
    okay = okay && RuledDiagnosticArchive::exportReport(capture->directory(), destination, report, screenshot, &error);
    if (!okay)
        QMessageBox::warning(window, QObject::tr("Report export failed"),
                             error.isEmpty() ? QObject::tr("Could not write report or screenshot.") : error);
    else
        QMessageBox::information(
            window, QObject::tr("Bug Report Saved"),
            QObject::tr("Saved %1\nReport ID: %2\nServer capture: %3")
                .arg(destination, reportId, serverId.isEmpty() ? QObject::tr("unavailable") : serverId));
}
} // namespace
void RuledBugReport::install(MainWindow *window, QMenu *menu)
{
    menu->addSeparator();
    auto *action = menu->addAction(QObject::tr("Report Bug…"));
    action->setObjectName("ruledReportBug");
    QObject::connect(action, &QAction::triggered, window, [window] { report(window); });
    auto *open = menu->addAction(QObject::tr("Open Bug Report…"));
    open->setObjectName("ruledOpenBugReport");
    QObject::connect(open, &QAction::triggered, window, [window] { RuledDiagnosticViewer::open(window); });
}
