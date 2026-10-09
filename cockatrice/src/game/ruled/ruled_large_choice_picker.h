#ifndef COCKATRICE_RULED_LARGE_CHOICE_PICKER_H
#define COCKATRICE_RULED_LARGE_CHOICE_PICKER_H

#include <QVector>
#include <QWidget>

class QLineEdit;
class QListWidget;
class QPushButton;

/// Searchable ruled-game picker for engine-authored high-cardinality branch choices.
class RuledLargeChoicePicker final : public QWidget
{
    Q_OBJECT

public:
    struct Option
    {
        int index = -1;
        QString label;
        bool enabled = false;
    };

    explicit RuledLargeChoicePicker(QWidget *parent = nullptr);
    void setOptions(const QVector<Option> &options);
    void clear();
    void retranslateUi();

signals:
    void optionRequested(int optionIndex);

private:
    void applyFilter(const QString &text);
    void updateConfirmEnabled();
    void confirmSelection();

    QLineEdit *searchEdit = nullptr;
    QListWidget *optionList = nullptr;
    QPushButton *confirmButton = nullptr;
};

#endif // COCKATRICE_RULED_LARGE_CHOICE_PICKER_H
