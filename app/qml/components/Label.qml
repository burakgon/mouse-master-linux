import QtQuick

Text {
    property int size: Theme.fontBody
    property bool strong: false
    property bool secondary: false
    property bool tertiary: false

    color: tertiary ? Theme.textTertiary : secondary ? Theme.textSecondary : Theme.text
    font.pixelSize: size
    font.weight: strong ? Font.DemiBold : Font.Normal
    elide: Text.ElideRight
    textFormat: Text.PlainText
    linkColor: Theme.accent
}
