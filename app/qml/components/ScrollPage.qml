import QtQuick
import QtQuick.Controls.Basic as C
import QtQuick.Layouts

// A vertically scrolling page with a centered, width limited column.
C.ScrollView {
    id: root
    default property alias content: column.data
    property int maxContentWidth: 820
    property int margins: 32
    contentWidth: availableWidth
    clip: true

    C.ScrollBar.vertical.policy: C.ScrollBar.AsNeeded

    ColumnLayout {
        id: column
        width: Math.min(root.availableWidth - root.margins * 2, root.maxContentWidth)
        x: (root.availableWidth - width) / 2
        y: root.margins
        spacing: 20
    }
    Item {
        // bottom padding
        y: column.y + column.height
        width: 1
        height: root.margins
    }
}
