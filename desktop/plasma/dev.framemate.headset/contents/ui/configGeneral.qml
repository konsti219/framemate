import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.kde.kcmutils as KCM

KCM.SimpleKCM {
    property alias cfg_host: host.text
    property alias cfg_port: port.value
    property alias cfg_token: token.text
    property alias cfg_pollSeconds: poll.value
    property real cfg_sleepDrainPerHour
    property string cfg_lastState
    property string cfg_lastOkMs

    Kirigami.FormLayout {
        QQC2.TextField {
            id: host
            Kirigami.FormData.label: i18n("Headset address:")
            placeholderText: "frame"
        }
        QQC2.SpinBox {
            id: port
            Kirigami.FormData.label: i18n("Port:")
            from: 1
            to: 65535
        }
        QQC2.TextField {
            id: token
            Kirigami.FormData.label: i18n("Token:")
            placeholderText: "framemate-agent token"
            echoMode: TextInput.Password
        }
        QQC2.SpinBox {
            id: poll
            Kirigami.FormData.label: i18n("Update every (s):")
            from: 2
            to: 600
        }
        QQC2.SpinBox {
            id: drain
            Kirigami.FormData.label: i18n("Drain while asleep (%/h, ×0.1):")
            from: 0
            to: 200
            value: Math.round(cfg_sleepDrainPerHour * 10)
            onValueModified: cfg_sleepDrainPerHour = value / 10
        }
    }
}
