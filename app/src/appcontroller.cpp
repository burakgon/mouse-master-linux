// SPDX-License-Identifier: GPL-3.0-or-later
#include "appcontroller.h"

#include <QAction>
#include <QCoreApplication>
#include <QIcon>
#include <QQuickWindow>

namespace {
const QString AppId = QStringLiteral("io.github.burakgon.MouseMaster");
const QString AppName = QStringLiteral("Mouse Master");
}

AppController::AppController(QObject *parent)
    : QObject(parent)
{
    m_trayAvailable = QSystemTrayIcon::isSystemTrayAvailable();
    m_tray.setIcon(QIcon::fromTheme(AppId, QIcon(QStringLiteral(":/icons/app/io.github.burakgon.MouseMaster.svg"))));
    m_tray.setToolTip(AppName);

    auto *open = m_menu.addAction(QStringLiteral("Open Mouse Master"));
    connect(open, &QAction::triggered, this, &AppController::activate);
    m_menu.addSeparator();
    auto *quitAction = m_menu.addAction(QStringLiteral("Quit"));
    connect(quitAction, &QAction::triggered, this, &AppController::quit);
    m_tray.setContextMenu(&m_menu);
    connect(&m_tray, &QSystemTrayIcon::activated, this, [this](QSystemTrayIcon::ActivationReason reason) {
        if (reason == QSystemTrayIcon::Trigger || reason == QSystemTrayIcon::DoubleClick) {
            activate();
        }
    });

    if (m_trayAvailable) {
        m_tray.show();
    }
}

void AppController::setWindow(QQuickWindow *window)
{
    m_window = window;
}

bool AppController::hideOnClose() const
{
    return m_trayAvailable;
}

void AppController::activate()
{
    if (!m_window) {
        return;
    }
    m_window->show();
    m_window->raise();
    m_window->requestActivate();
}

void AppController::quit()
{
    QCoreApplication::quit();
}
