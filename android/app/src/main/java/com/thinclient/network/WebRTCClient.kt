package com.thinclient.network

import android.content.Context
import android.util.Log
import org.webrtc.*
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicBoolean

class WebRTCClient(
    private val context: Context,
    private val signalingClient: SignalingClient,
    private val callId: String,
    private val jwtToken: String,
    private val baseUrl: String
) {
    // Instance-level resources (per-call)
    private var peerConnection: PeerConnection? = null
    private var localAudioTrack: AudioTrack? = null
    private var audioSource: AudioSource? = null
    

    // Debug stats
    @Volatile var iceConnectionState = "NEW"
    @Volatile var peerConnectionState = "NEW"
    @Volatile var signalingState = "STABLE"
    @Volatile var iceGatheringState = "NEW"
    @Volatile var remoteTrackReceived = false
    @Volatile var isSpeaker = true
    @Volatile var localAudioEnabled = false
    @Volatile var localTrackCreated = false
    @Volatile var permissionGranted = false
    @Volatile var audioSourceCreated = false
    @Volatile var senderPresent = false

    val localCandidatesGenerated = AtomicInteger(0)
    val remoteCandidatesReceived = AtomicInteger(0)
    val remoteCandidatesAdded = AtomicInteger(0)
    val candidateAddFailures = AtomicInteger(0)

    // Lifecycle guard
    private val closed = AtomicBoolean(false)
    private val statsTimer: java.util.Timer = java.util.Timer("webrtc-stats", true)

    private val pendingIceCandidates = mutableListOf<IceCandidate>()
    @Volatile private var isRemoteDescriptionSet = false

    // Callback to notify UI of real WebRTC connection state changes
    var onConnectionStateChange: ((String) -> Unit)? = null

    companion object {
        // Process-level singletons — initialized ONCE, reused for every call.
        // Disposing these between calls is the #1 cause of 3rd/5th call failures.
        @Volatile private var factoryInitialized = false
        @Volatile private var sharedFactory: PeerConnectionFactory? = null
        private val sharedEglBase: EglBase by lazy { EglBase.create() }
        private val initLock = Any()

        fun ensureInitialized(context: Context) {
            synchronized(initLock) {
                if (factoryInitialized) return
                val initOpts = PeerConnectionFactory.InitializationOptions
                    .builder(context.applicationContext)
                    .setEnableInternalTracer(false)
                    .createInitializationOptions()
                PeerConnectionFactory.initialize(initOpts)

                val adm = org.webrtc.audio.JavaAudioDeviceModule.builder(context.applicationContext)
                    .setUseHardwareAcousticEchoCanceler(true)
                    .setUseHardwareNoiseSuppressor(true)
                    .createAudioDeviceModule()

                sharedFactory = PeerConnectionFactory.builder()
                    .setOptions(PeerConnectionFactory.Options())
                    .setAudioDeviceModule(adm)
                    .setVideoEncoderFactory(DefaultVideoEncoderFactory(sharedEglBase.eglBaseContext, true, true))
                    .setVideoDecoderFactory(DefaultVideoDecoderFactory(sharedEglBase.eglBaseContext))
                    .createPeerConnectionFactory()

                adm.release() // ADM can be released after factory creation
                factoryInitialized = true
                Log.i("WebRTCClient", "Shared PeerConnectionFactory created (once)")
            }
        }
    }

    init {
        Log.i("WebRTCClient", "[$callId] Initializing WebRTCClient")
        ensureInitialized(context)   // no-op if already done
        createPeerConnection()
        // Run audio track creation on a background thread with retry
        // so previous call's audio hardware has time to fully release
        Thread {
            Thread.sleep(150) // brief pause for audio HW to release between calls
            checkPermissionAndCreateMediaTracks()
        }.apply { isDaemon = true; start() }
    }


    private fun checkPermissionAndCreateMediaTracks() {
        val granted = androidx.core.content.ContextCompat.checkSelfPermission(
            context, android.Manifest.permission.RECORD_AUDIO
        ) == android.content.pm.PackageManager.PERMISSION_GRANTED

        permissionGranted = granted

        if (!granted) {
            Log.e("WebRTCClient", "[$callId] RECORD_AUDIO denied — audio track NOT created")
            return
        }

        try {
            val am = context.getSystemService(Context.AUDIO_SERVICE) as android.media.AudioManager
            am.mode = android.media.AudioManager.MODE_IN_COMMUNICATION
            am.isSpeakerphoneOn = true
            isSpeaker = true

            val constraints = MediaConstraints()

            // Retry up to 3 times if audio source returns null
            // (can happen if previous call's hardware hasn't fully released)
            var attempts = 0
            while (audioSource == null && attempts < 3) {
                if (attempts > 0) {
                    Log.w("WebRTCClient", "[$callId] audioSource null on attempt $attempts, retrying...")
                    Thread.sleep(300)
                }
                audioSource = sharedFactory?.createAudioSource(constraints)
                attempts++
            }
            audioSourceCreated = (audioSource != null)

            if (audioSource == null) {
                Log.e("WebRTCClient", "[$callId] createAudioSource returned null after $attempts attempts!")
                return
            }

            localAudioTrack = sharedFactory?.createAudioTrack("audio0", audioSource)
            localAudioTrack?.setEnabled(true)
            localAudioEnabled = true
            localTrackCreated = (localAudioTrack != null)

            if (localAudioTrack != null) {
                val transceivers = peerConnection?.transceivers
                var trackAttached = false
                if (transceivers != null) {
                    for (t in transceivers) {
                        if (t.mediaType == MediaStreamTrack.MediaType.MEDIA_TYPE_AUDIO) {
                            t.sender.setTrack(localAudioTrack, false)
                            trackAttached = true
                            senderPresent = true
                            break
                        }
                    }
                }
                if (!trackAttached) {
                    val sender = peerConnection?.addTrack(localAudioTrack, listOf("stream0"))
                    senderPresent = (sender != null)
                }
                Log.i("WebRTCClient", "[$callId] Audio track added. Sender present: $senderPresent")
            }

        } catch (e: Exception) {
            Log.e("WebRTCClient", "[$callId] Failed to create tracks", e)
            localTrackCreated = false
        }
    }



    private fun createPeerConnection() {
        val iceServers = listOf(
            PeerConnection.IceServer.builder("stun:stun.l.google.com:19302").createIceServer(),
            PeerConnection.IceServer.builder("stun:stun1.l.google.com:19302").createIceServer(),
            PeerConnection.IceServer.builder("stun:stun2.l.google.com:19302").createIceServer()
        )
        buildPeerConnection(iceServers)
    }

    private fun buildPeerConnection(iceServers: List<PeerConnection.IceServer>) {
        val rtcConfig = PeerConnection.RTCConfiguration(iceServers).apply {
            sdpSemantics = PeerConnection.SdpSemantics.UNIFIED_PLAN
            continualGatheringPolicy = PeerConnection.ContinualGatheringPolicy.GATHER_CONTINUALLY
            iceTransportsType = PeerConnection.IceTransportsType.ALL
            // Aggressive ICE: faster connection on tricky NATs
            bundlePolicy = PeerConnection.BundlePolicy.MAXBUNDLE
            rtcpMuxPolicy = PeerConnection.RtcpMuxPolicy.REQUIRE
        }

        peerConnection = sharedFactory?.createPeerConnection(rtcConfig, object : PeerConnection.Observer {
            override fun onSignalingChange(s: PeerConnection.SignalingState?) {
                signalingState = s?.name ?: "UNKNOWN"
                Log.i("WebRTCClient", "[$callId] SignalingState: $signalingState")
            }

            override fun onIceConnectionChange(s: PeerConnection.IceConnectionState?) {
                iceConnectionState = s?.name ?: "UNKNOWN"
                Log.i("WebRTCClient", "[$callId] ICE connection: $iceConnectionState")
                // Drive call state from real ICE state
                when (s) {
                    PeerConnection.IceConnectionState.CONNECTED,
                    PeerConnection.IceConnectionState.COMPLETED ->
                        onConnectionStateChange?.invoke("CONNECTED")
                    PeerConnection.IceConnectionState.FAILED -> {
                        onConnectionStateChange?.invoke("FAILED")
                        // NOTE: Do NOT call close() or dispose() here!
                        // Calling peerConnection.dispose() from within its own callback
                        // causes a native crash. MainActivity handles cleanup via onConnectionStateChange.
                    }
                    PeerConnection.IceConnectionState.DISCONNECTED ->
                        onConnectionStateChange?.invoke("RECONNECTING")
                    PeerConnection.IceConnectionState.CHECKING ->
                        onConnectionStateChange?.invoke("CONNECTING")
                    else -> {}
                }
            }

            override fun onConnectionChange(s: PeerConnection.PeerConnectionState?) {
                peerConnectionState = s?.name ?: "UNKNOWN"
                Log.i("WebRTCClient", "[$callId] PeerConnection state: $peerConnectionState")
            }

            override fun onIceGatheringChange(s: PeerConnection.IceGatheringState?) {
                iceGatheringState = s?.name ?: "UNKNOWN"
                Log.i("WebRTCClient", "[$callId] ICE gathering: $iceGatheringState")
            }

            override fun onIceConnectionReceivingChange(r: Boolean) {}

            override fun onIceCandidate(candidate: IceCandidate?) {
                if (candidate == null) return
                // Don't send candidates if this call is already closed
                if (closed.get()) {
                    Log.d("WebRTCClient", "[$callId] Skipping ICE candidate — call closed")
                    return
                }
                val n = localCandidatesGenerated.incrementAndGet()
                val type = candidate.sdp.substringAfter("typ ", "").substringBefore(" ")
                val ip = candidate.sdp.split(" ").getOrNull(4) ?: "unknown"
                Log.i("WebRTCClient", "[$callId] Local ICE candidate #$n (type=$type, IP=$ip)")

                val iceJson = com.google.gson.JsonObject().apply {
                    addProperty("sdpMid", candidate.sdpMid)
                    addProperty("sdpMLineIndex", candidate.sdpMLineIndex)
                    addProperty("sdp", candidate.sdp)
                }
                val message = com.google.gson.JsonObject().apply {
                    addProperty("type", "call_ice")
                    addProperty("request_id", java.util.UUID.randomUUID().toString())
                    addProperty("session_id", "ignored")
                    addProperty("call_id", callId)
                    addProperty("seq", 1)
                    add("payload", com.google.gson.JsonObject().apply { add("candidate", iceJson) })
                }
                signalingClient.send(message.toString())
            }

            override fun onIceCandidatesRemoved(candidates: Array<out IceCandidate>?) {}
            override fun onAddStream(stream: MediaStream?) {}
            override fun onRemoveStream(stream: MediaStream?) {}
            override fun onDataChannel(dc: DataChannel?) {}
            override fun onRenegotiationNeeded() {}
            override fun onAddTrack(receiver: RtpReceiver?, streams: Array<out MediaStream>?) {
                val track = receiver?.track()
                if (track is AudioTrack) {
                    remoteTrackReceived = true
                    track.setEnabled(true)
                    Log.i("WebRTCClient", "[$callId] Remote AudioTrack received and enabled")
                }
            }

        override fun onTrack(transceiver: RtpTransceiver?) {
            val track = transceiver?.receiver?.track()
            if (track is AudioTrack) {
                remoteTrackReceived = true
                track.setEnabled(true)
                Log.i("WebRTCClient", "[$callId] Remote AudioTrack via onTrack, enabled")
            }
        }
        })

        // Force the SDP to include an audio transceiver, so we can negotiate it 
        // even if the physical AudioTrack hasn't finished initializing on the background thread.
        val init = RtpTransceiver.RtpTransceiverInit(
            RtpTransceiver.RtpTransceiverDirection.SEND_RECV,
            listOf("stream0")
        )
        peerConnection?.addTransceiver(MediaStreamTrack.MediaType.MEDIA_TYPE_AUDIO, init)
        Log.i("WebRTCClient", "[$callId] PeerConnection created, ICE servers: ${iceServers.size}")
    }

    /** Fetch TURN credentials then restart ICE with them */
    fun fetchTurnAndRestartIce() {
        val okHttp = okhttp3.OkHttpClient()
        val req = okhttp3.Request.Builder()
            .url("$baseUrl/v1/calls/$callId/turn-credentials")
            .get()
            .addHeader("Authorization", "Bearer $jwtToken")
            .build()

        okHttp.newCall(req).enqueue(object : okhttp3.Callback {
            override fun onFailure(call: okhttp3.Call, e: java.io.IOException) {
                Log.e("WebRTCClient", "[$callId] TURN credential fetch failed: ${e.message}")
                // Proceed without TURN — may still work on same LAN
            }

            override fun onResponse(call: okhttp3.Call, response: okhttp3.Response) {
                if (!response.isSuccessful) {
                    Log.e("WebRTCClient", "[$callId] TURN credential fetch HTTP ${response.code}")
                    return
                }
                val body = response.body?.string() ?: return
                try {
                    val json = com.google.gson.JsonParser.parseString(body).asJsonObject
                    val url = json.get("url").asString
                    val username = json.get("username").asString
                    val credential = json.get("credential").asString

                    Log.i("WebRTCClient", "[$callId] TURN credentials received, url=$url")

                    val iceServers = listOf(
                        PeerConnection.IceServer.builder("stun:stun.l.google.com:19302").createIceServer(),
                        PeerConnection.IceServer.builder(url)
                            .setUsername(username)
                            .setPassword(credential)
                            .createIceServer(),
                        // fallback public TURN
                        PeerConnection.IceServer.builder("turn:openrelay.metered.ca:80")
                            .setUsername("openrelayproject")
                            .setPassword("openrelayproject")
                            .createIceServer(),
                        PeerConnection.IceServer.builder("turn:openrelay.metered.ca:443")
                            .setUsername("openrelayproject")
                            .setPassword("openrelayproject")
                            .createIceServer(),
                        PeerConnection.IceServer.builder("turn:openrelay.metered.ca:443?transport=tcp")
                            .setUsername("openrelayproject")
                            .setPassword("openrelayproject")
                            .createIceServer()
                    )

                    val rtcConfig = PeerConnection.RTCConfiguration(iceServers).apply {
                        sdpSemantics = PeerConnection.SdpSemantics.UNIFIED_PLAN
                        continualGatheringPolicy = PeerConnection.ContinualGatheringPolicy.GATHER_CONTINUALLY
                        iceTransportsType = PeerConnection.IceTransportsType.ALL
                    }
                    peerConnection?.setConfiguration(rtcConfig)
                    Log.i("WebRTCClient", "[$callId] ICE config updated with TURN server")
                } catch (e: Exception) {
                    Log.e("WebRTCClient", "[$callId] Failed to parse TURN credentials", e)
                }
            }
        })
    }

    fun setMuted(mute: Boolean) {
        localAudioTrack?.setEnabled(!mute)
        localAudioEnabled = !mute
        Log.i("WebRTCClient", "[$callId] Muted=$mute")
    }

    fun startCall() {
        Log.i("WebRTCClient", "[$callId] startCall() — creating offer")
        onConnectionStateChange?.invoke("NEGOTIATING")
        val constraints = MediaConstraints().apply {
            mandatory.add(MediaConstraints.KeyValuePair("OfferToReceiveAudio", "true"))
        }
        peerConnection?.createOffer(object : SdpObserver {
            override fun onCreateSuccess(sdp: SessionDescription?) {
                if (sdp == null) { Log.e("WebRTCClient", "[$callId] Offer SDP is null"); return }
                Log.i("WebRTCClient", "[$callId] Offer created, setting local description")
                peerConnection?.setLocalDescription(object : SdpObserver {
                    override fun onCreateSuccess(p0: SessionDescription?) {}
                    override fun onSetSuccess() {
                        Log.i("WebRTCClient", "[$callId] Local description set (offer)")
                        val sdpJson = com.google.gson.JsonObject().apply {
                            addProperty("type", "offer")
                            addProperty("sdp", sdp.description)
                        }
                        val message = com.google.gson.JsonObject().apply {
                            addProperty("type", "call_offer")
                            addProperty("request_id", java.util.UUID.randomUUID().toString())
                            addProperty("session_id", "ignored")
                            addProperty("call_id", callId)
                            addProperty("seq", 1)
                            add("payload", sdpJson)
                        }
                        signalingClient.send(message.toString())
                    }
                    override fun onCreateFailure(e: String?) { Log.e("WebRTCClient", "[$callId] setLocalDescription create fail: $e") }
                    override fun onSetFailure(e: String?) { Log.e("WebRTCClient", "[$callId] setLocalDescription fail: $e") }
                }, sdp)
            }
            override fun onSetSuccess() {}
            override fun onCreateFailure(e: String?) { Log.e("WebRTCClient", "[$callId] createOffer failed: $e") }
            override fun onSetFailure(e: String?) {}
        }, constraints)
    }

    fun handleOffer(sdp: String) {
        Log.i("WebRTCClient", "[$callId] handleOffer() — setting remote description")
        onConnectionStateChange?.invoke("NEGOTIATING")
        val remote = SessionDescription(SessionDescription.Type.OFFER, sdp)
        peerConnection?.setRemoteDescription(object : SdpObserver {
            override fun onCreateSuccess(p0: SessionDescription?) {}
            override fun onSetSuccess() {
                Log.i("WebRTCClient", "[$callId] Remote description (offer) set, creating answer")
                synchronized(pendingIceCandidates) {
                    isRemoteDescriptionSet = true
                    for (c in pendingIceCandidates) {
                        if (peerConnection?.addIceCandidate(c) == true) remoteCandidatesAdded.incrementAndGet()
                        else candidateAddFailures.incrementAndGet()
                    }
                    pendingIceCandidates.clear()
                }
                val constraints = MediaConstraints().apply {
                    mandatory.add(MediaConstraints.KeyValuePair("OfferToReceiveAudio", "true"))
                }
                peerConnection?.createAnswer(object : SdpObserver {
                    override fun onCreateSuccess(answerSdp: SessionDescription?) {
                        if (answerSdp == null) { Log.e("WebRTCClient", "[$callId] Answer SDP is null"); return }
                        peerConnection?.setLocalDescription(object : SdpObserver {
                            override fun onCreateSuccess(p0: SessionDescription?) {}
                            override fun onSetSuccess() {
                                Log.i("WebRTCClient", "[$callId] Local description set (answer)")
                                val sdpJson = com.google.gson.JsonObject().apply {
                                    addProperty("type", "answer")
                                    addProperty("sdp", answerSdp.description)
                                }
                                val message = com.google.gson.JsonObject().apply {
                                    addProperty("type", "call_answer")
                                    addProperty("request_id", java.util.UUID.randomUUID().toString())
                                    addProperty("session_id", "ignored")
                                    addProperty("call_id", callId)
                                    addProperty("seq", 1)
                                    add("payload", sdpJson)
                                }
                                signalingClient.send(message.toString())
                            }
                            override fun onCreateFailure(e: String?) { Log.e("WebRTCClient", "[$callId] answer setLocal create fail: $e") }
                            override fun onSetFailure(e: String?) { Log.e("WebRTCClient", "[$callId] answer setLocal fail: $e") }
                        }, answerSdp)
                    }
                    override fun onSetSuccess() {}
                    override fun onCreateFailure(e: String?) { Log.e("WebRTCClient", "[$callId] createAnswer failed: $e") }
                    override fun onSetFailure(e: String?) {}
                }, constraints)
            }
            override fun onCreateFailure(e: String?) { Log.e("WebRTCClient", "[$callId] setRemote create fail: $e") }
            override fun onSetFailure(e: String?) { Log.e("WebRTCClient", "[$callId] setRemote fail: $e") }
        }, remote)
    }

    fun handleAnswer(sdp: String) {
        Log.i("WebRTCClient", "[$callId] handleAnswer() — setting remote description")
        val remote = SessionDescription(SessionDescription.Type.ANSWER, sdp)
        peerConnection?.setRemoteDescription(object : SdpObserver {
            override fun onCreateSuccess(p0: SessionDescription?) {}
            override fun onSetSuccess() { 
                Log.i("WebRTCClient", "[$callId] Remote description (answer) set")
                synchronized(pendingIceCandidates) {
                    isRemoteDescriptionSet = true
                    for (c in pendingIceCandidates) {
                        if (peerConnection?.addIceCandidate(c) == true) remoteCandidatesAdded.incrementAndGet()
                        else candidateAddFailures.incrementAndGet()
                    }
                    pendingIceCandidates.clear()
                }
            }
            override fun onCreateFailure(e: String?) { Log.e("WebRTCClient", "[$callId] answer setRemote create fail: $e") }
            override fun onSetFailure(e: String?) { Log.e("WebRTCClient", "[$callId] answer setRemote fail: $e") }
        }, remote)
    }

    fun handleIceCandidate(sdpMid: String, sdpMLineIndex: Int, sdp: String) {
        val n = remoteCandidatesReceived.incrementAndGet()
        val type = sdp.substringAfter("typ ", "").substringBefore(" ")
        val ip = sdp.split(" ").getOrNull(4) ?: "unknown"
        Log.i("WebRTCClient", "[$callId] Remote ICE candidate #$n received (type=$type, IP=$ip)")
        val candidate = IceCandidate(sdpMid, sdpMLineIndex, sdp)
        
        synchronized(pendingIceCandidates) {
            if (isRemoteDescriptionSet) {
                val result = peerConnection?.addIceCandidate(candidate)
                if (result == true) {
                    remoteCandidatesAdded.incrementAndGet()
                } else {
                    candidateAddFailures.incrementAndGet()
                    Log.w("WebRTCClient", "[$callId] addIceCandidate returned false")
                }
            } else {
                Log.i("WebRTCClient", "[$callId] Remote description not set yet, buffering candidate #$n")
                pendingIceCandidates.add(candidate)
            }
        }
    }

    fun startStatsTimer() {
        statsTimer.schedule(object : java.util.TimerTask() {
            override fun run() {
                if (closed.get()) return
                peerConnection?.getStats(object : RTCStatsCollectorCallback {
                    override fun onStatsDelivered(report: RTCStatsReport) {
                        if (closed.get()) return
                        var bytesSent = 0L
                        var packetsSent = 0L
                        var bytesReceived = 0L
                        var packetsReceived = 0L

                        for (stat in report.statsMap.values) {
                            when (stat.type) {
                                "outbound-rtp" -> {
                                    bytesSent = (stat.members["bytesSent"] as? java.math.BigInteger)?.toLong()
                                        ?: (stat.members["bytesSent"] as? Long) ?: bytesSent
                                    packetsSent = (stat.members["packetsSent"] as? Long) ?: packetsSent
                                }
                                "inbound-rtp" -> {
                                    bytesReceived = (stat.members["bytesReceived"] as? java.math.BigInteger)?.toLong()
                                        ?: (stat.members["bytesReceived"] as? Long) ?: bytesReceived
                                    packetsReceived = (stat.members["packetsReceived"] as? Long) ?: packetsReceived
                                }
                            }
                        }

                        (context as? android.app.Activity)?.runOnUiThread {
                            if (closed.get()) return@runOnUiThread
                            val view = (context as? android.app.Activity)
                                ?.findViewById<android.view.View>(android.R.id.content)
                                ?.findViewWithTag<com.thinclient.WebRTCDebugView>("audio_visualizer")
                            view?.apply {
                                this.iceState = iceConnectionState
                                this.pcState = peerConnectionState
                                this.remoteTrackReceived = this@WebRTCClient.remoteTrackReceived
                                this.isSpeaker = this@WebRTCClient.isSpeaker
                                this.localAudioEnabled = this@WebRTCClient.localAudioEnabled
                                this.localTrackCreated = this@WebRTCClient.localTrackCreated
                                this.permissionGranted = this@WebRTCClient.permissionGranted
                                this.audioSourceCreated = this@WebRTCClient.audioSourceCreated
                                this.senderPresent = this@WebRTCClient.senderPresent
                                this.localCandidatesGenerated = this@WebRTCClient.localCandidatesGenerated.get()
                                this.remoteCandidatesReceived = this@WebRTCClient.remoteCandidatesReceived.get()
                                this.remoteCandidatesAdded = this@WebRTCClient.remoteCandidatesAdded.get()
                                updateStats(bytesSent, packetsSent, bytesReceived, packetsReceived)
                            }
                        }
                    }
                })
            }
        }, 0, 500)
    }

    fun close() {
        if (!closed.compareAndSet(false, true)) return
        statsTimer.cancel()

        try {
            val am = context.getSystemService(Context.AUDIO_SERVICE) as android.media.AudioManager
            am.mode = android.media.AudioManager.MODE_NORMAL
            am.isSpeakerphoneOn = false
        } catch (e: Exception) { /* ignore */ }

        // Dispose only per-call resources.
        // sharedFactory and sharedEglBase are process-level singletons — NEVER dispose them.
        // Disposing them was the root cause of 3rd/5th call failures.
        localAudioTrack?.dispose()
        audioSource?.dispose()
        

        
        peerConnection?.dispose()   // dispose() fully releases ICE/DTLS state
        Log.i("WebRTCClient", "[$callId] Closed (factory kept alive for next call)")
    }
}
