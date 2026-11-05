// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once

#include <QObject>
#include <QMenu>
#include <QSystemTrayIcon>

class QQuickWindow;

class AppController : public QObject
{
    Q_OBJECT
public:
    explicit AppController(QObject *parent = nullptr);

    void setWindow(QQuickWindow *window);

    Q_INVOKABLE bool hideOnClose() const;
    Q_INVOKABLE void activate();
    Q_INVOKABLE void quit();

private:
    QMenu m_menu;
    QSystemTrayIcon m_tray;
    QQuickWindow *m_window = nullptr;
    bool m_trayAvailable = false;
};
