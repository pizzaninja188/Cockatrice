#include "ruled_commander_editor.h"

#include "../../interface/widgets/cards/deck_preview_card_picture_widget.h"
#include "../../interface/widgets/deck_editor/deck_state_manager.h"

#include <QHBoxLayout>
#include <QLabel>
#include <QLineEdit>
#include <QPushButton>
#include <QVBoxLayout>
#include <libcockatrice/card/database/card_database_manager.h>
#include <libcockatrice/card/database/card_database_querier.h>

RuledCommanderEditor::RuledCommanderEditor(DeckStateManager *state, QWidget *parent) : QGroupBox(parent), manager(state)
{
    setObjectName("commanderEditor");
    auto *layout = new QVBoxLayout(this);
    cards = new QWidget(this);
    cardsLayout = new QHBoxLayout(cards);
    cardsLayout->setContentsMargins(0, 0, 0, 0);
    layout->addWidget(cards);
    auto *input = new QHBoxLayout;
    auto *name = new QLineEdit(this);
    name->setObjectName("commanderName");
    name->setPlaceholderText(tr("Commander name"));
    auto *add = new QPushButton(tr("Add commander"), this);
    add->setObjectName("addCommander");
    input->addWidget(name);
    input->addWidget(add);
    layout->addLayout(input);
    const auto addCommander = [this, name] {
        const auto text = name->text().trimmed();
        if (text.isEmpty()) {
            return;
        }
        auto declarations = manager->getDeckList().getCommanders();
        declarations.append(CardRef{text, {}});
        manager->setCommanders(declarations);
        name->clear();
    };
    connect(add, &QPushButton::clicked, this, addCommander);
    connect(name, &QLineEdit::returnPressed, this, addCommander);
    connect(manager, &DeckStateManager::metadataModified, this, &RuledCommanderEditor::refresh, Qt::QueuedConnection);
    connect(manager, &DeckStateManager::deckReplaced, this, &RuledCommanderEditor::refresh, Qt::QueuedConnection);
    refresh();
}

void RuledCommanderEditor::refresh()
{
    while (auto *item = cardsLayout->takeAt(0)) {
        delete item->widget();
        delete item;
    }
    const auto commanders = manager->getDeckList().getCommanders();
    setTitle(tr("Commander (%1)").arg(commanders.size()));
    if (commanders.isEmpty()) {
        cardsLayout->addWidget(new QLabel(tr("No commander selected"), cards));
    }
    for (int index = 0; index < commanders.size(); ++index) {
        const auto card = commanders[index];
        auto *entry = new QWidget(cards);
        auto *layout = new QVBoxLayout(entry);
        layout->setContentsMargins(0, 0, 0, 0);
        auto *picture = new DeckPreviewCardPictureWidget(entry, true);
        picture->setFixedSize(72, 102);
        picture->setCard(CardDatabaseManager::query()->getCard(card));
        picture->setOverlayText(card.name);
        picture->setToolTip(card.name);
        layout->addWidget(picture);
        auto *returnCard = new QPushButton(tr("To mainboard"), entry);
        connect(returnCard, &QPushButton::clicked, this, [this, index] { manager->returnCommanderToMainboard(index); });
        layout->addWidget(returnCard);
        auto *remove = new QPushButton(tr("Remove"), entry);
        connect(remove, &QPushButton::clicked, this, [this, index] {
            auto declarations = manager->getDeckList().getCommanders();
            if (index >= declarations.size()) {
                return;
            }
            declarations.removeAt(index);
            manager->setCommanders(declarations);
        });
        layout->addWidget(remove);
        cardsLayout->addWidget(entry);
    }
    cardsLayout->addStretch();
}
