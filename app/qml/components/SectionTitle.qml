import QtQuick
import QtQuick.Layouts

RowLayout {
    property string text
    property string subtitle: ""
    spacing: 8
    Layout.fillWidth: true
    ColumnLayout {
        spacing: 2
        Layout.fillWidth: true
        Label {
            text: parent.parent.text
            size: Theme.fontHeadline
            strong: true
        }
        Label {
            visible: parent.parent.subtitle !== ""
            text: parent.parent.subtitle
            secondary: true
            size: Theme.fontSmall
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }
    }
}
