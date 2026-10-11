// SPDX-License-Identifier: GPL-3.0-or-later
#include "iconprovider.h"

#include <QFile>
#include <QIcon>
#include <QPainter>
#include <QSvgRenderer>

LucideIconProvider::LucideIconProvider()
    : QQuickImageProvider(QQuickImageProvider::Image)
{
}

QImage LucideIconProvider::requestImage(const QString &id, QSize *size, const QSize &requestedSize)
{
    const QString name = id.section('/', 0, 0);
    const QString color = id.section('/', 1, 1);
    QFile file(QStringLiteral(":/icons/lucide/%1.svg").arg(name));
    if (!file.open(QIODevice::ReadOnly)) {
        file.setFileName(QStringLiteral(":/icons/lucide/circle-dot.svg"));
        if (!file.open(QIODevice::ReadOnly)) {
            return {};
        }
    }
    QByteArray svg = file.readAll();
    if (!color.isEmpty()) {
        svg.replace("currentColor", ('#' + color).toLatin1());
    }
    const QSize out = requestedSize.isValid() && !requestedSize.isEmpty() ? requestedSize : QSize(24, 24);
    QImage image(out, QImage::Format_ARGB32_Premultiplied);
    image.fill(Qt::transparent);
    QSvgRenderer renderer(svg);
    QPainter painter(&image);
    painter.setRenderHint(QPainter::Antialiasing);
    renderer.render(&painter);
    painter.end();
    if (size) {
        *size = out;
    }
    return image;
}

ThemeIconProvider::ThemeIconProvider()
    : QQuickImageProvider(QQuickImageProvider::Pixmap)
{
}

QPixmap ThemeIconProvider::requestPixmap(const QString &id, QSize *size, const QSize &requestedSize)
{
    const QSize out = requestedSize.isValid() && !requestedSize.isEmpty() ? requestedSize : QSize(48, 48);
    QIcon icon;
    if (id.startsWith('/')) {
        icon = QIcon(id);
    } else {
        icon = QIcon::fromTheme(id);
    }
    if (icon.isNull()) {
        icon = QIcon::fromTheme(QStringLiteral("application-x-executable"));
    }
    if (size) {
        *size = out;
    }
    return icon.pixmap(out);
}
