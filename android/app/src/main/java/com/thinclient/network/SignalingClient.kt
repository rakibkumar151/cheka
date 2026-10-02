package com.thinclient.network

import android.util.Log
import okhttp3.*
import okio.ByteString

class SignalingClient(private val url: String, private val token: String, private val onMessage: (String) -> Unit) {
    private val client = OkHttpClient()
    private var webSocket: WebSocket? = null

    fun connect() {
        val request = Request.Builder()
            .url(url)
            .addHeader("Authorization", "Bearer $token")
            .build()

        webSocket = client.newWebSocket(request, object : WebSocketListener() {
            override fun onOpen(webSocket: WebSocket, response: Response) {
                Log.i("SignalingClient", "WebSocket connected successfully.")
            }

            override fun onMessage(webSocket: WebSocket, text: String) {
                Log.i("SignalingClient", "Received message: $text")
                onMessage(text)
            }

            override fun onMessage(webSocket: WebSocket, bytes: ByteString) {
                Log.i("SignalingClient", "Received bytes: ${bytes.hex()}")
            }

            override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
                Log.i("SignalingClient", "WebSocket closing: $code $reason")
                webSocket.close(1000, null)
            }

            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                Log.e("SignalingClient", "WebSocket failure", t)
            }
        })
    }

    fun send(message: String) {
        webSocket?.send(message)
    }

    fun disconnect() {
        webSocket?.close(1000, "User logout")
    }
}
