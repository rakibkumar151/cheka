package com.thinclient

import android.content.Context
import android.graphics.Color
import android.graphics.Typeface
import android.widget.LinearLayout
import android.widget.TextView
import android.Manifest
import android.content.pm.PackageManager
import androidx.core.content.ContextCompat

class WebRTCDebugView(context: Context) : LinearLayout(context) {
    private val tv: TextView = TextView(context)

    // Fields updated by WebRTCClient stats timer
    var iceState: String = "NEW"
    var pcState: String = "NEW"
    var remoteTrackReceived: Boolean = false
    var isSpeaker: Boolean = false
    var localAudioEnabled: Boolean = false
    var localTrackCreated: Boolean = false
    var permissionGranted: Boolean = false
    var audioSourceCreated: Boolean = false
    var senderPresent: Boolean = false
    var localCandidatesGenerated: Int = 0
    var remoteCandidatesReceived: Int = 0
    var remoteCandidatesAdded: Int = 0

    init {
        orientation = VERTICAL
        setBackgroundColor(Color.parseColor("#111111"))
        tv.setTextColor(Color.parseColor("#00FF88"))
        tv.textSize = 11f
        tv.typeface = Typeface.MONOSPACE
        tv.setPadding(20, 20, 20, 20)
        addView(tv)
        updateStats(0, 0, 0, 0)
    }

    fun updateStats(bytesSent: Long, packetsSent: Long, bytesReceived: Long, packetsReceived: Long) {
        val sys = ContextCompat.checkSelfPermission(context, Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED

        val callStateLabel = when {
            iceState == "FAILED" || pcState == "FAILED" -> "FAILED"
            iceState == "CONNECTED" || iceState == "COMPLETED" -> "CONNECTED"
            iceState == "CHECKING" || pcState == "CONNECTING" -> "CONNECTING"
            iceState == "DISCONNECTED" -> "RECONNECTING"
            localTrackCreated -> "NEGOTIATING"
            else -> "INITIALIZING"
        }

        val text = buildString {
            appendLine("── CALL STATE ──────────────────")
            appendLine("  $callStateLabel")
            appendLine()
            appendLine("── LOCAL AUDIO ──────────────────")
            appendLine("  Permission : ${yn(sys)}")
            appendLine("  Source     : ${if (audioSourceCreated) "CREATED" else "FAILED"}")
            appendLine("  Track      : ${if (localTrackCreated) "CREATED" else "FAILED"}")
            appendLine("  Enabled    : ${yn(localAudioEnabled)}")
            appendLine("  Sender     : ${if (senderPresent) "PRESENT" else "MISSING"}")
            appendLine("  Pkt sent   : $packetsSent")
            appendLine("  Bytes sent : $bytesSent")
            appendLine()
            appendLine("── REMOTE AUDIO ─────────────────")
            appendLine("  Track      : ${if (remoteTrackReceived) "RECEIVED" else "NOT RECEIVED"}")
            appendLine("  Pkt recv   : $packetsReceived")
            appendLine("  Bytes recv : $bytesReceived")
            appendLine()
            appendLine("── ICE ──────────────────────────")
            appendLine("  Gathering  : $iceState")
            appendLine("  Connection : $iceState")
            appendLine("  Local cand : $localCandidatesGenerated")
            appendLine("  Remote cand: $remoteCandidatesReceived")
            appendLine("  Added      : $remoteCandidatesAdded")
            appendLine()
            appendLine("── PEER CONNECTION ──────────────")
            appendLine("  $pcState")
            appendLine()
            appendLine("── AUDIO OUTPUT ─────────────────")
            appendLine("  ${if (isSpeaker) "SPEAKER" else "EARPIECE"}")
        }
        tv.text = text
    }

    private fun yn(b: Boolean) = if (b) "YES" else "NO"
}
