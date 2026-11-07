#ifndef RULED_INTERACTION_SETTINGS_WIDGET_H
#define RULED_INTERACTION_SETTINGS_WIDGET_H

#include <QCheckBox>
#include <QCoreApplication>
#include <QEvent>
#include <QGridLayout>
#include <QLabel>
#include <QSpinBox>
#include <QWidget>
#include <libcockatrice/settings/cards_display_settings.h>

class RuledInteractionSettingsWidget : public QWidget
{
    QCheckBox hover;
    QLabel strengthLabel;
    QSpinBox strength;

    void retranslate()
    {
        hover.setText(QCoreApplication::translate("RuledInteractionSettings", "Dim unrelated cards on hover"));
        strengthLabel.setText(QCoreApplication::translate("RuledInteractionSettings", "Card dimming strength:"));
    }

protected:
    void changeEvent(QEvent *event) override
    {
        if (event->type() == QEvent::LanguageChange) {
            retranslate();
        }
        QWidget::changeEvent(event);
    }

public:
    explicit RuledInteractionSettingsWidget(CardsDisplaySettings &settings, QWidget *parent = nullptr) : QWidget(parent)
    {
        auto *layout = new QGridLayout(this);
        layout->setContentsMargins(0, 0, 0, 0);
        hover.setChecked(settings.getRuledHoverDimming());
        strength.setRange(0, 100);
        strength.setSuffix(QStringLiteral("%"));
        strength.setValue(settings.getRuledDimPercent());
        strengthLabel.setBuddy(&strength);
        layout->addWidget(&hover, 0, 0, 1, 2);
        layout->addWidget(&strengthLabel, 1, 0);
        layout->addWidget(&strength, 1, 1);
        connect(&hover, &QCheckBox::toggled, &settings, &CardsDisplaySettings::setRuledHoverDimming);
        connect(&strength, qOverload<int>(&QSpinBox::valueChanged), &settings,
                &CardsDisplaySettings::setRuledDimPercent);
        retranslate();
    }
};

#endif
