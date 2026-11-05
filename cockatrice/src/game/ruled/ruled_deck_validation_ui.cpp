#include "ruled_deck_validation_ui.h"

#include "../../interface/widgets/deck_editor/deck_state_manager.h"
#include "../../interface/widgets/tabs/abstract_tab_deck_editor.h"
#include "../../interface/widgets/tabs/tab_supervisor.h"

#include <QComboBox>
#include <QDialog>
#include <QDialogButtonBox>
#include <QLabel>
#include <QPushButton>
#include <QTextEdit>
#include <QVBoxLayout>
#include <libcockatrice/network/client/abstract/abstract_client.h>
#include <libcockatrice/protocol/pb/response_deck_download.pb.h>
#include <libcockatrice/protocol/pb/session_commands.pb.h>
#include <libcockatrice/protocol/pending_command.h>
#include <libcockatrice/protocol/ruled_deck_validation_report.h>

namespace RuledDeckValidationUi
{
void open(AbstractTabDeckEditor *editor)
{
    auto *dialog = new QDialog(editor);
    dialog->setAttribute(Qt::WA_DeleteOnClose);
    dialog->setWindowTitle(QObject::tr("Validate deck"));
    dialog->resize(620, 440);
    auto *layout = new QVBoxLayout(dialog);
    auto *format = new QComboBox(dialog);
    format->addItem(QObject::tr("Standard"), ruled::v1::GAME_FORMAT_STANDARD);
    format->addItem(QObject::tr("Commander"), ruled::v1::GAME_FORMAT_COMMANDER);
    auto *manager = editor->deckStateManager;
    format->setCurrentIndex(manager->getDeckList().getGameFormat().compare("commander", Qt::CaseInsensitive) == 0 ? 1
                                                                                                                  : 0);
    layout->addWidget(new QLabel(QObject::tr("Validate against the server's current format catalogue:"), dialog));
    layout->addWidget(format);
    auto *result = new QTextEdit(dialog);
    result->setReadOnly(true);
    result->setPlainText(QObject::tr(
        "Choose a format and validate. Format legality and executable card support are reported separately."));
    layout->addWidget(result);
    auto *buttons = new QDialogButtonBox(QDialogButtonBox::Close, dialog);
    auto *validate = buttons->addButton(QObject::tr("Validate"), QDialogButtonBox::ActionRole);
    layout->addWidget(buttons);
    QObject::connect(buttons, &QDialogButtonBox::rejected, dialog, &QDialog::close);
    const auto stale = [result] {
        result->setPlainText(QObject::tr("Deck or format changed. Validate again for a current result."));
    };
    QObject::connect(manager, &DeckStateManager::deckModified, dialog, stale);
    QObject::connect(manager, &DeckStateManager::metadataModified, dialog, stale);
    QObject::connect(manager, &DeckStateManager::deckReplaced, dialog, stale);
    QObject::connect(format, &QComboBox::currentIndexChanged, dialog, stale);
    QObject::connect(manager, &QObject::destroyed, dialog, &QDialog::close);
    QObject::connect(validate, &QPushButton::clicked, dialog, [editor, manager, dialog, format, result, validate] {
        auto *client = editor->getTabSupervisor()->getServerClient();
        if (!client || client->getStatus() != StatusLoggedIn) {
            result->setPlainText(QObject::tr("Connect to a server to validate this deck."));
            return;
        }
        const auto native = manager->getDeckList().writeToString_Native();
        const auto selected = format->currentData().toInt();
        Command_RuledValidateDeck request;
        request.set_deck(native.toStdString());
        request.set_format(static_cast<ruled::v1::GameFormat>(selected));
        auto *pending = client->prepareSessionCommand(request);
        validate->setEnabled(false);
        result->setPlainText(QObject::tr("Validating…"));
        QObject::connect(pending, &PendingCommand::finished, dialog,
                         [manager, format, result, validate, native, selected](const Response &response) {
                             validate->setEnabled(true);
                             if (native != manager->getDeckList().writeToString_Native() ||
                                 selected != format->currentData().toInt()) {
                                 result->setPlainText(
                                     QObject::tr("This response is out of date. Validate the current deck again."));
                                 return;
                             }
                             if (response.response_code() != Response::RespOk ||
                                 !response.HasExtension(Response_RuledDeckValidation::ext)) {
                                 result->setPlainText(QObject::tr("The server could not validate this deck. Check that "
                                                                  "it is running the matching rules engine."));
                                 return;
                             }
                             result->setPlainText(RuledDeckValidationReport::describe(
                                 response.GetExtension(Response_RuledDeckValidation::ext).validation()));
                         });
        client->sendCommand(pending);
    });
    dialog->show();
}
} // namespace RuledDeckValidationUi
