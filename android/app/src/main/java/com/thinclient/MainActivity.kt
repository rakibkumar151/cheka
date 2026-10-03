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
    private var myUid: String = ""
    private var activeCallId: String? = null
    private val deadCallIds = mutableSetOf<String>()
    
    // Local state for UI consistency
    private var isCameraOn = false
    private var isMuted = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(android.widget.TextView(this).apply { text = "Authenticating..." })

        if (androidx.core.content.ContextCompat.checkSelfPermission(this, android.Manifest.permission.RECORD_AUDIO)
            != android.content.pm.PackageManager.PERMISSION_GRANTED) {
            androidx.core.app.ActivityCompat.requestPermissions(
                this,
                arrayOf(android.Manifest.permission.RECORD_AUDIO),
                1
            )
        }

        myUid = "usr_" + UUID.randomUUID().toString().replace("-", "").substring(0, 12)
        Log.i("MainActivity", "Generated UID: $myUid")
        authenticate(myUid)
    }

    // ─── Central call teardown ─────────────────────────────────────────────────
    // Call this from ANYWHERE: End Call button, ICE failed, call_end msg, crash
    private fun patchSchema(schema: com.thinclient.model.SduiSchema): com.thinclient.model.SduiSchema {
        val patchedComponents = schema.components.map { comp ->
            when (comp.action) {
                "call.camera_off", "call.camera_on" -> comp.copy(
                    action = if (isCameraOn) "call.camera_off" else "call.camera_on",
                    text = if (isCameraOn) "Turn Camera Off" else "Turn Camera On"
                )
                "call.mute", "call.unmute" -> comp.copy(
                    action = if (isMuted) "call.unmute" else "call.mute",
                    text = if (isMuted) "Unmute" else "Mute"
                )
                else -> comp
            }
        }
        return schema.copy(components = patchedComponents)
    }

    private fun endCallAndGoHome(callId: String? = null) {
        Log.i("MainActivity", "endCallAndGoHome callId=$callId activeCallId=$activeCallId")
        isCameraOn = false
        isMuted = false

        if (callId != null) {
            if (deadCallIds.contains(callId)) {
                Log.i("MainActivity", "Call $callId is already dead, ignoring teardown.")
                return
            }
            deadCallIds.add(callId)
        }

        // If this teardown is for a specific call, but we have moved on to a NEW call, ignore it!
        if (callId != null && activeCallId != null && callId != activeCallId) {
            Log.i("MainActivity", "Ignoring teardown for $callId because active call is $activeCallId")
            return
        }

        activeCallId = null

        // 1. Close WebRTC — safe to call even if already closed
        val wc = webRTCClient
        webRTCClient = null          // null FIRST so no re-entrant calls
        wc?.close()

        // 2. Notify server (fire-and-forget, failures are silent)
        if (!callId.isNullOrBlank() && jwtToken.isNotBlank()) {
            val reqBody = "".toRequestBody("application/json".toMediaType())
            val req = Request.Builder()
                .url("$baseUrl/v1/calls/$callId/reject")
                .post(reqBody)
                .addHeader("Authorization", "Bearer $jwtToken")
                .build()
            client.newCall(req).enqueue(object : Callback {
                override fun onFailure(call: Call, e: IOException) { /* silent */ }
                override fun onResponse(call: Call, response: Response) { response.close() }
            })
        }

        // 3. Navigate back to My UID home screen
        runOnUiThread { showMyUidScreen() }
    }

    // ─── Home screen with user's UID ──────────────────────────────────────────
    private fun showMyUidScreen() {
        val schema = SduiSchema(
            schema_version = 1,
            screen = "home",
            revision = 1,
            title = "My UID",
            components = listOf(
                com.thinclient.model.SduiComponent(
                    id = "lbl_uid",
                    type = "text",
                    text = myUid
                ),
                com.thinclient.model.SduiComponent(
                    id = "btn_copy",
                    type = "button",
                    text = "Copy UID",
                    action = "copy_text",
                    data = myUid
                ),
                com.thinclient.model.SduiComponent(
                    id = "inp_target",
                    type = "input",
                    text = "Enter target UID"
                ),
                com.thinclient.model.SduiComponent(
                    id = "btn_call",
                    type = "button",
                    text = "Start Audio Call",
                    action = "call.start_audio",
                    data = "inp_target"
                )
            )
        )
        val view = sduiRenderer.render(schema)
        setContentView(view)
    }

    private fun authenticate(uid: String) {
        val reqBody = gson.toJson(mapOf("user_id" to uid)).toRequestBody("application/json".toMediaType())
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

    private fun createWebRTCClient(callId: String, isVideoCall: Boolean): com.thinclient.network.WebRTCClient {
        val wc = com.thinclient.network.WebRTCClient(
            this, signalingClient!!, callId, jwtToken, baseUrl, isVideoCall
        )
        wc.onConnectionStateChange = { state ->
            Log.i("MainActivity", "[$callId] WebRTC state → $state")
            runOnUiThread {
                // Update debug view if present
                val view = findViewById<android.view.View>(android.R.id.content)
                    ?.findViewWithTag<WebRTCDebugView>("audio_visualizer")
                view?.iceState = wc.iceConnectionState
                view?.pcState = wc.peerConnectionState
                view?.updateStats(0, 0, 0, 0)

                // Auto-navigate home on any terminal failure
                // Crucial: Only trigger if THIS WebRTCClient is still the active one
                if (state == "FAILED" && webRTCClient === wc) {
                    Log.i("MainActivity", "[$callId] ICE/Connection FAILED → auto going home")
                    endCallAndGoHome(callId)
                }
            }
        }
        wc.startStatsTimer()
        return wc
    }

    private fun initializeServices(token: String) {
        actionDispatcher = ActionDispatcher(this, baseUrl, token, { schema ->
            runOnUiThread {
                val view = sduiRenderer.render(patchSchema(schema))
                setContentView(view)
                webRTCClient?.attachVideoRenderers()
            }
        }, { newCallId ->
            runOnUiThread { activeCallId = newCallId }
        })

        sduiRenderer = SduiRenderer(this) { action, data ->
            when (action) {
                "call.mute"   -> {
                    isMuted = true
                    webRTCClient?.setMuted(true)
                }
                "call.unmute" -> {
                    isMuted = false
                    webRTCClient?.setMuted(false)
                }
                "call.camera_on" -> {
                    isCameraOn = true
                    webRTCClient?.toggleVideo(true)
                }
                "call.camera_off" -> {
                    isCameraOn = false
                    webRTCClient?.toggleVideo(false)
                }
                "call.camera_switch" -> webRTCClient?.switchCamera()
                "call.end"    -> {
                    // End Call button pressed — close everything and go home
                    val callId = data
                    endCallAndGoHome(callId)
                    return@SduiRenderer      // don't also dispatch to ActionDispatcher
                }
            }
            actionDispatcher.dispatch(action, data)
        }

        signalingClient = SignalingClient(wsUrl, token) { text ->
            try {
                val message = gson.fromJson(text, JsonObject::class.java)
                val msgType = message.get("type")?.asString
                val callId  = message.get("call_id")?.asString

                if (callId != null && deadCallIds.contains(callId)) {
                    Log.w("MainActivity", "Ignoring late message $msgType for already dead call $callId")
                    return@SignalingClient
                }

                // Filter out late messages from old calls
                if (callId != null && activeCallId != null && callId != activeCallId) {
                    Log.w("MainActivity", "Ignoring late message $msgType for dead call $callId (active: $activeCallId)")
                    return@SignalingClient
                }
                
                // Track new incoming calls
                if (callId != null && (msgType == "call_incoming" || msgType == "call_accepted")) {
                    activeCallId = callId
                }

                when {
                    msgType == "call_accepted" && callId != null -> {
                        val payload = message.get("payload")
                        var schema: SduiSchema? = null
                        if (payload != null && !payload.isJsonNull) {
                            schema = gson.fromJson(payload, SduiSchema::class.java)
                        }
                        runOnUiThread {
                            if (webRTCClient == null) {
                                val isVideo = schema?.components?.any { it.type == "local_video" || it.type == "remote_video" } ?: false
                                webRTCClient = createWebRTCClient(callId, isVideo)
                                webRTCClient?.startCall()
                            }
                        }
                        if (schema != null) {
                            runOnUiThread {
                                val view = sduiRenderer.render(patchSchema(schema))
                                setContentView(view)
                                webRTCClient?.attachVideoRenderers()
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
                                webRTCClient?.attachVideoRenderers()
                            }
                        }
                    }

                    // Both call_rejected and call_end → clean up and go home
                    // IMPORTANT: Do NOT render any payload here — it would overwrite the My UID screen
                    msgType == "call_rejected" || msgType == "call_end" -> {
                        endCallAndGoHome(callId)
                    }

                    msgType == "call_offer" && callId != null -> {
                        val payload = message.getAsJsonObject("payload")
                        val sdp = payload.get("sdp").asString
                        runOnUiThread {
                            if (webRTCClient == null) {
                                // For callee, the active call screen (and its schema) might already be rendered
                                // Let's try to detect if it's a video call by checking if we have a video renderer in the view tree
                                val root = findViewById<android.view.View>(android.R.id.content)
                                val isVideo = root?.findViewWithTag<android.view.View>("local_video_renderer") != null ||
                                              root?.findViewWithTag<android.view.View>("remote_video_renderer") != null
                                webRTCClient = createWebRTCClient(callId, isVideo)
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
                        val payload   = message.getAsJsonObject("payload")
                        val candidate = payload.getAsJsonObject("candidate")
                        val sdpMid        = candidate.get("sdpMid").asString
                        val sdpMLineIndex = candidate.get("sdpMLineIndex").asInt
                        val sdp           = candidate.get("sdp").asString
                        runOnUiThread {
                            // Only add candidate if we still have an active client for THIS call
                            if (webRTCClient != null) {
                                webRTCClient?.handleIceCandidate(sdpMid, sdpMLineIndex, sdp)
                            }
                        }
                    }
                }
            } catch (e: Exception) {
                Log.e("MainActivity", "Failed to parse websocket message: ${e.message}", e)
            }
        }
        signalingClient?.connect()

        // Show home screen immediately after init
        showMyUidScreen()
    }

    override fun onDestroy() {
        super.onDestroy()
        webRTCClient?.close()
        signalingClient?.disconnect()
    }
}
