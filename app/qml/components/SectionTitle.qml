import QtQuick
import QtQuick.Layouts

RowLayout {
    id: root
    property string text
    property string subtitle: ""
    spacing: 8
    Layout.fillWidth: true
    ColumnLayout {
        spacing: 2
        Layout.fillWidth: true
        Label {
            text: root.text
            size: Theme.fontHeadline
            strong: true
        }
        Label {
            visible: root.subtitle !== ""
            text: root.subtitle
            secondary: true
            size: Theme.fontSmall
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }
    }
}
