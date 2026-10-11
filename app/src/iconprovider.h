// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once

#include <QQuickImageProvider>

/// `image://lucide/<name>/<rrggbb>`: a bundled Lucide icon, tinted.
class LucideIconProvider : public QQuickImageProvider
{
public:
    LucideIconProvider();
    QImage requestImage(const QString &id, QSize *size, const QSize &requestedSize) override;
};

/// `image://theme/<icon name or path>`: an icon from the system icon theme.
class ThemeIconProvider : public QQuickImageProvider
{
public:
    ThemeIconProvider();
    QPixmap requestPixmap(const QString &id, QSize *size, const QSize &requestedSize) override;
};
