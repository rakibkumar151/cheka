package com.thinclient.actions

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.util.Log
import com.google.gson.Gson
import com.thinclient.model.SduiComponent
import com.thinclient.model.SduiSchema
import okhttp3.*
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.RequestBody.Companion.toRequestBody
import java.io.IOException

class ActionDispatcher(
    private val context: Context,
    private val baseUrl: String,
    private val jwtToken: String,
    private val onUiUpdate: (SduiSchema) -> Unit
) {
    private val client = OkHttpClient()
    private val gson = Gson()
    
    private val allowlist = setOf(
        "navigate",
        "call.start_audio",
        "call.start_video",
        "call.accept",
        "call.reject",
        "call.end",
        "call.mute",
        "call.unmute",
        "call.camera_on",
        "call.camera_off",
        "call.camera_switch",
        "retry",
        "logout",
        "copy_text"
    )

    fun dispatch(action: String, data: String? = null) {
        if (!allowlist.contains(action)) {
            Log.e("Security", "REJECTED UNKNOWN ACTION: $action. SAFE SECURITY EVENT LOGGED.")
            return
        }

        Log.i("ActionDispatcher", "Executing allowed action: $action")

        when (action) {
            "copy_text" -> {
                data?.let {
                    val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                    val clip = ClipData.newPlainText("Copied UID", it)
                    clipboard.setPrimaryClip(clip)
                    Log.i("ActionDispatcher", "Copied to clipboard: ${it.hashCode()}") // Redacted log
                }
            }
            "call.start_audio", "call.start_video" -> {
                if (data.isNullOrBlank()) {
                    showError("Target UID cannot be empty")
                    return
                }
                val kind = if (action == "call.start_audio") "audio" else "video"
                Log.i("ActionDispatcher", "Initiating $kind call to target hash: ${data.hashCode()}")
                
                // Show INITIATING state
                showCallState("INITIATING...")

                val jsonStr = gson.toJson(mapOf(
                    "target_user_id" to data,
                    "call_type" to kind
                ))

                val reqBody = jsonStr.toRequestBody("application/json".toMediaType())
                val request = Request.Builder()
                    .url("$baseUrl/v1/calls")
                    .post(reqBody)
                    .addHeader("Authorization", "Bearer $jwtToken")
                    .build()

                val startTime = System.currentTimeMillis()

                client.newCall(request).enqueue(object : Callback {
                    override fun onFailure(call: Call, e: IOException) {
                        Log.e("ActionDispatcher", "HTTP call failed", e)
                        showError("CALL FAILED: Network error")
                    }

                    override fun onResponse(call: Call, response: Response) {
                        val latency = System.currentTimeMillis() - startTime
                        Log.i("ActionDispatcher", "HTTP response: ${response.code} (Latency: ${latency}ms)")
                        
                        if (response.isSuccessful) {
                            val resStr = response.body?.string()
                            if (resStr != null) {
                                try {
                                    val map = gson.fromJson(resStr, Map::class.java)
                                    val callId = map["call_id"] as? String
                                    val state = map["state"] as? String
                                    Log.i("ActionDispatcher", "Call created: $callId, state: $state")
                                    showCallState(state ?: "RINGING")
                                } catch (e: Exception) {
                                    Log.e("ActionDispatcher", "Failed to parse JSON response", e)
                                }
                            }
                        } else {
                            Log.e("ActionDispatcher", "API error: ${response.code}")
                            showError("CALL FAILED: ${response.code}")
                        }
                    }
                })
            }
            "call.accept", "call.reject", "call.end", "call.mute", "call.unmute", "call.camera_on", "call.camera_off", "call.camera_switch" -> {
                val callId = data
                if (callId.isNullOrBlank()) {
                    showError("Call ID missing")
                    return
                }
                
                if (action == "call.accept" || action == "call.reject") {
                    val isAccept = action == "call.accept"
                    Log.i("ActionDispatcher", "${if (isAccept) "Accepting" else "Rejecting"} call: $callId")
                    showCallState(if (isAccept) "CONNECTING..." else "IDLE")
                    
                    val reqBody = "".toRequestBody("application/json".toMediaType())
                    val urlAction = if (isAccept) "accept" else "reject"
                    val request = Request.Builder()
                        .url("$baseUrl/v1/calls/$callId/$urlAction")
                        .post(reqBody)
                        .addHeader("Authorization", "Bearer $jwtToken")
                        .build()

                    client.newCall(request).enqueue(object : Callback {
                        override fun onFailure(call: Call, e: IOException) {
                            Log.e("ActionDispatcher", "Failed to $urlAction call", e)
                            showError("Network error")
                        }
                        override fun onResponse(call: Call, response: Response) {
                            Log.i("ActionDispatcher", "Call $urlAction response: ${response.code}")
                        }
                    })
                } else if (action == "call.end") {
                    // Quick fix for end call
                    val reqBody = "".toRequestBody("application/json".toMediaType())
                    val request = Request.Builder()
                        .url("$baseUrl/v1/calls/$callId/reject")
                        .post(reqBody)
                        .addHeader("Authorization", "Bearer $jwtToken")
                        .build()
                    client.newCall(request).enqueue(object: Callback {
                        override fun onFailure(call: Call, e: IOException) {}
                        override fun onResponse(call: Call, response: Response) {}
                    })
                } else {
                    // Send other actions to new endpoint
                    val reqBody = gson.toJson(mapOf("action_id" to action)).toRequestBody("application/json".toMediaType())
                    val request = Request.Builder()
                        .url("$baseUrl/v1/calls/$callId/action")
                        .post(reqBody)
                        .addHeader("Authorization", "Bearer $jwtToken")
                        .build()
                    client.newCall(request).enqueue(object : Callback {
                        override fun onFailure(call: Call, e: IOException) {
                            Log.e("ActionDispatcher", "Failed to send action $action", e)
                        }
                        override fun onResponse(call: Call, response: Response) {
                            Log.i("ActionDispatcher", "Call action $action response: ${response.code}")
                        }
                    })
                }
            }
        }
    }

    private fun showCallState(stateStr: String) {
        val schema = SduiSchema(
            schema_version = 1,
            screen = "call_state",
            revision = 1,
            title = "Call State",
            components = listOf(
                SduiComponent(
                    id = "lbl_state",
                    type = "text",
                    text = stateStr
                )
            )
        )
        onUiUpdate(schema)
    }

    private fun showError(msg: String) {
        val schema = SduiSchema(
            schema_version = 1,
            screen = "error",
            revision = 1,
            title = "Error",
            components = listOf(
                SduiComponent(
                    id = "lbl_err",
                    type = "text",
                    text = msg,
                    style = "error"
                ),
                SduiComponent(
                    id = "btn_retry",
                    type = "button",
                    text = "DISMISS",
                    action = "retry"
                )
            )
        )
        onUiUpdate(schema)
    }
}
