import QtQuick
import QtQuick.Layouts

RowLayout {
    id: root
    property string title
    property string subtitle: ""
    default property alias trailing: trailingRow.data
    Layout.fillWidth: true
    spacing: 16
    ColumnLayout {
        Layout.fillWidth: true
        spacing: 4
        Label {
            text: root.title
            size: Theme.fontTitle + 4
            strong: true
            Layout.fillWidth: true
        }
        Label {
            visible: root.subtitle !== ""
            text: root.subtitle
            secondary: true
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }
    }
    Row {
        id: trailingRow
        spacing: 8
        Layout.alignment: Qt.AlignVCenter
    }
}
