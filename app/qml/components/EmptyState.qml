import QtQuick
import QtQuick.Layouts

ColumnLayout {
    id: root
    property string iconName: "mouse"
    property string title
    property string text: ""
    default property alias actions: actionRow.data
    spacing: 14

    Rectangle {
        Layout.alignment: Qt.AlignHCenter
        width: 84
        height: 84
        radius: 42
        color: Theme.accentAlpha(Theme.dark ? 0.14 : 0.1)
        Icon {
            anchors.centerIn: parent
            name: root.iconName
            size: 40
            color: Theme.accent
        }
    }
    Label {
        text: root.title
        size: Theme.fontTitle
        strong: true
        horizontalAlignment: Text.AlignHCenter
        Layout.alignment: Qt.AlignHCenter
    }
    Label {
        visible: root.text !== ""
        text: root.text
        secondary: true
        wrapMode: Text.WordWrap
        elide: Text.ElideNone
        horizontalAlignment: Text.AlignHCenter
        Layout.alignment: Qt.AlignHCenter
        Layout.maximumWidth: 460
        Layout.preferredWidth: 460
        lineHeight: 1.25
    }
    Row {
        id: actionRow
        spacing: 10
        Layout.alignment: Qt.AlignHCenter
        Layout.topMargin: 6
    }
}
