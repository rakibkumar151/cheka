package com.thinclient

import android.os.Bundle
import android.util.Log
import androidx.appcompat.app.AppCompatActivity
import com.thinclient.actions.ActionDispatcher
import com.thinclient.model.SduiSchema
import com.thinclient.network.SignalingClient
import com.thinclient.renderer.SduiRenderer
import com.google.gson.Gson
import com.google.gson.JsonObject
import okhttp3.*
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.RequestBody.Companion.toRequestBody
import java.io.IOException
import java.util.UUID

class MainActivity : AppCompatActivity() {
    private lateinit var sduiRenderer: SduiRenderer
    private lateinit var actionDispatcher: ActionDispatcher
    private var signalingClient: SignalingClient? = null
    private var webRTCClient: com.thinclient.network.WebRTCClient? = null
    private val gson = Gson()
    private val client = OkHttpClient()
    private val baseUrl = "https://cheka.onrender.com"
    private val wsUrl = "wss://cheka.onrender.com/v1/ws"
    private var jwtToken: String = ""

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(android.widget.TextView(this).apply { text = "Authenticating..." })

        // Request only RECORD_AUDIO — do NOT request CAMERA for audio calls
        if (androidx.core.content.ContextCompat.checkSelfPermission(this, android.Manifest.permission.RECORD_AUDIO)
            != android.content.pm.PackageManager.PERMISSION_GRANTED) {
            androidx.core.app.ActivityCompat.requestPermissions(
                this,
                arrayOf(android.Manifest.permission.RECORD_AUDIO),
                1
            )
        }

        val myUid = "usr_" + UUID.randomUUID().toString().replace("-", "").substring(0, 12)
        Log.i("MainActivity", "Generated UID: $myUid")
        authenticate(myUid)
    }

    private fun authenticate(myUid: String) {
        val reqBody = gson.toJson(mapOf("user_id" to myUid)).toRequestBody("application/json".toMediaType())
        val req = Request.Builder().url("$baseUrl/v1/auth/anonymous").post(reqBody).build()
        client.newCall(req).enqueue(object : Callback {
            override fun onFailure(call: Call, e: IOException) {
                Log.e("MainActivity", "Auth failed", e)
                runOnUiThread { setContentView(android.widget.TextView(this@MainActivity).apply { text = "Auth Failed" }) }
            }
            override fun onResponse(call: Call, response: Response) {
                if (response.isSuccessful) {
                    val res = response.body?.string()
                    val map = gson.fromJson(res, Map::class.java)
                    val token = map["token"] as String
                    jwtToken = token
                    runOnUiThread { initializeServices(token) }
                }
            }
        })
    }

    private fun createWebRTCClient(callId: String): com.thinclient.network.WebRTCClient {
        val wc = com.thinclient.network.WebRTCClient(
            this, signalingClient!!, callId, jwtToken, baseUrl
        )
        wc.onConnectionStateChange = { state ->
            Log.i("MainActivity", "[$callId] WebRTC state → $state")
            // Update debug panel call state label in real time
            runOnUiThread {
                val view = findViewById<android.view.View>(android.R.id.content)
                    ?.findViewWithTag<WebRTCDebugView>("audio_visualizer")
                view?.iceState = wc.iceConnectionState
                view?.pcState = wc.peerConnectionState
                view?.updateStats(0, 0, 0, 0)
            }
        }
        wc.fetchTurnAndRestartIce()
        wc.startStatsTimer()
        return wc
    }

    private fun initializeServices(token: String) {
        actionDispatcher = ActionDispatcher(this, baseUrl, token) { schema ->
            runOnUiThread {
                val view = sduiRenderer.render(schema)
                setContentView(view)
            }
        }

        sduiRenderer = SduiRenderer(this) { action, data ->
            when (action) {
                "call.mute" -> webRTCClient?.setMuted(true)
                "call.unmute" -> webRTCClient?.setMuted(false)
            }
            actionDispatcher.dispatch(action, data)
        }

        signalingClient = SignalingClient(wsUrl, token) { text ->
            try {
                val message = gson.fromJson(text, JsonObject::class.java)
                val msgType = message.get("type")?.asString
                val callId = message.get("call_id")?.asString

                when {
                    msgType == "call_accepted" && callId != null -> {
                        // Caller path: server told us callee accepted → we create offer
                        runOnUiThread {
                            if (webRTCClient == null) {
                                webRTCClient = createWebRTCClient(callId)
                                webRTCClient?.startCall()
                            }
                        }
                        // Also render SDUI if payload present
                        val payload = message.get("payload")
                        if (payload != null && !payload.isJsonNull) {
                            val schema = gson.fromJson(payload, SduiSchema::class.java)
                            runOnUiThread {
                                val view = sduiRenderer.render(schema)
                                setContentView(view)
                            }
                        }
                    }

                    msgType == "sdui_update" || msgType == "call_incoming" -> {
                        val payload = message.get("payload")
                        if (payload != null && !payload.isJsonNull) {
                            val schema = gson.fromJson(payload, SduiSchema::class.java)
                            runOnUiThread {
                                val view = sduiRenderer.render(schema)
                                setContentView(view)
                            }
                        }
                    }

                    msgType == "call_rejected" || msgType == "call_end" -> {
                        runOnUiThread {
                            webRTCClient?.close()
                            webRTCClient = null
                        }
                        val payload = message.get("payload")
                        if (payload != null && !payload.isJsonNull) {
                            val schema = gson.fromJson(payload, SduiSchema::class.java)
                            runOnUiThread {
                                val view = sduiRenderer.render(schema)
                                setContentView(view)
                            }
                        }
                    }

                    msgType == "call_offer" && callId != null -> {
                        // Callee path: we receive the offer from caller
                        val payload = message.getAsJsonObject("payload")
                        val sdp = payload.get("sdp").asString
                        runOnUiThread {
                            if (webRTCClient == null) {
                                webRTCClient = createWebRTCClient(callId)
                            }
                            webRTCClient?.handleOffer(sdp)
                        }
                    }

                    msgType == "call_answer" && callId != null -> {
                        val payload = message.getAsJsonObject("payload")
                        val sdp = payload.get("sdp").asString
                        runOnUiThread { webRTCClient?.handleAnswer(sdp) }
                    }

                    msgType == "call_ice" && callId != null -> {
                        val payload = message.getAsJsonObject("payload")
                        val candidate = payload.getAsJsonObject("candidate")
                        val sdpMid = candidate.get("sdpMid").asString
                        val sdpMLineIndex = candidate.get("sdpMLineIndex").asInt
                        val sdp = candidate.get("sdp").asString
                        // ICE candidates must NOT be added on UI thread to avoid blocking
                        // But ensure webRTCClient exists
                        runOnUiThread {
                            if (webRTCClient == null) {
                                webRTCClient = createWebRTCClient(callId)
                            }
                            webRTCClient?.handleIceCandidate(sdpMid, sdpMLineIndex, sdp)
                        }
                    }
                }
            } catch (e: Exception) {
                Log.e("MainActivity", "Failed to parse websocket message: ${e.message}", e)
            }
        }
        signalingClient?.connect()
    }

    override fun onDestroy() {
        super.onDestroy()
        webRTCClient?.close()
        signalingClient?.disconnect()
    }
}
