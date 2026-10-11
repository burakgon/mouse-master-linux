import QtQuick
import QtQuick.Controls.Basic as C
import QtQuick.Layouts

// A centered dialog with a dimmed backdrop.
C.Popup {
    id: root
    property string title
    property string subtitle: ""
    default property alias body: bodyColumn.data
    property alias footer: footerRow.data
    property int preferredWidth: 480

    parent: C.Overlay.overlay
    anchors.centerIn: parent
    width: Math.min(preferredWidth, (parent ? parent.width : preferredWidth) - 48)
    height: Math.min(implicitHeight, (parent ? parent.height : 600) - 48)
    modal: true
    focus: true
    padding: 0
    closePolicy: C.Popup.CloseOnEscape | C.Popup.CloseOnPressOutside

    C.Overlay.modal: Rectangle {
        color: Theme.overlay
        Behavior on opacity { NumberAnimation { duration: Theme.anim } }
    }

    enter: Transition {
        NumberAnimation { property: "opacity"; from: 0; to: 1; duration: Theme.anim }
        NumberAnimation { property: "scale"; from: 0.96; to: 1; duration: Theme.anim; easing.type: Easing.OutCubic }
    }
    exit: Transition {
        NumberAnimation { property: "opacity"; from: 1; to: 0; duration: Theme.animFast }
    }

    background: Rectangle {
        radius: Theme.radiusLarge + 2
        color: Theme.raised
        border.width: 1
        border.color: Theme.strokeStrong
    }

    contentItem: ColumnLayout {
        spacing: 0
        ColumnLayout {
            Layout.fillWidth: true
            Layout.margins: 24
            Layout.bottomMargin: 8
            spacing: 4
            Label {
                text: root.title
                size: Theme.fontTitle - 2
                strong: true
                Layout.fillWidth: true
            }
            Label {
                visible: root.subtitle !== ""
                text: root.subtitle
                secondary: true
                wrapMode: Text.WordWrap
                elide: Text.ElideNone
                Layout.fillWidth: true
            }
        }
        ColumnLayout {
            id: bodyColumn
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.leftMargin: 24
            Layout.rightMargin: 24
            Layout.topMargin: 8
            spacing: 12
        }
        RowLayout {
            id: footerRow
            Layout.fillWidth: true
            Layout.margins: 20
            Layout.topMargin: 16
            spacing: 10
            Item { Layout.fillWidth: true }
        }
    }
}
