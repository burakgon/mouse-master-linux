// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once

#include <QObject>
#include <QMenu>
#include <QSystemTrayIcon>

class QQuickWindow;

class AppController : public QObject
{
    Q_OBJECT
    Q_PROPERTY(bool trayAvailable READ trayAvailable CONSTANT)
    Q_PROPERTY(bool startOnLogin READ startOnLogin NOTIFY preferencesChanged)
    Q_PROPERTY(bool startMinimized READ startMinimized NOTIFY preferencesChanged)
public:
    explicit AppController(bool startMinimizedRequested, QObject *parent = nullptr);

    bool trayAvailable() const { return m_trayAvailable; }
    bool startOnLogin() const { return m_startOnLogin; }
    bool startMinimized() const { return m_startMinimized; }

    void setWindow(QQuickWindow *window);
    bool shouldStartMinimized() const { return m_startMinimizedRequested && m_trayAvailable; }

    Q_INVOKABLE bool setStartOnLogin(bool enabled);
    Q_INVOKABLE bool setStartMinimized(bool enabled);
    Q_INVOKABLE bool hideOnClose() const;
    Q_INVOKABLE void activate();
    Q_INVOKABLE void quit();

Q_SIGNALS:
    void preferencesChanged();

private:
    bool writeAutostartEntry(bool enabled, bool startMinimized) const;
    void persistPreferences() const;

    QMenu m_menu;
    QSystemTrayIcon m_tray;
    QQuickWindow *m_window = nullptr;
    bool m_trayAvailable = false;
    bool m_startOnLogin = false;
    bool m_startMinimized = false;
    bool m_startMinimizedRequested = false;
    bool m_quitting = false;
};
