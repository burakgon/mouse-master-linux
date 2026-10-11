import QtQuick
import QtQuick.Layouts

RowLayout {
    property string title
    property string subtitle: ""
    default property alias trailing: trailingRow.data
    Layout.fillWidth: true
    spacing: 16
    ColumnLayout {
        Layout.fillWidth: true
        spacing: 4
        Label {
            text: parent.parent.title
            size: Theme.fontTitle + 4
            strong: true
            Layout.fillWidth: true
        }
        Label {
            visible: parent.parent.subtitle !== ""
            text: parent.parent.subtitle
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
