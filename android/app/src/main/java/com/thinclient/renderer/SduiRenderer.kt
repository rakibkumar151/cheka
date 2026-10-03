package com.thinclient.renderer

import android.content.Context
import android.graphics.Color
import android.view.View
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.TextView
import com.thinclient.model.SduiComponent
import com.thinclient.model.SduiSchema

class SduiRenderer(private val context: Context, private val actionDispatcher: (String, String?) -> Unit) {

    private val inputViews = mutableMapOf<String, EditText>()

    fun render(schema: SduiSchema): View {
        inputViews.clear()

        if (schema.screen == "active_video_call") {
            return buildActiveVideoCallLayout(schema)
        }

        val root = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(32, 32, 32, 32)
        }
        
        schema.title?.let {
            val titleView = TextView(context).apply {
                text = it
                textSize = 24f
                setTextColor(Color.BLACK)
                setPadding(0, 0, 0, 32)
            }
            root.addView(titleView)
        }

        val componentsList = schema.components ?: emptyList()
        componentsList.forEach { component ->
            val view = buildComponent(component)
            if (view != null) {
                root.addView(view)
            }
        }
        
        return root
    }

    private fun buildComponent(component: SduiComponent): View? {
        return when (component.type) {
            "audio_visualizer" -> com.thinclient.WebRTCDebugView(context).apply {
                id = android.view.View.generateViewId()
                tag = "audio_visualizer"
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT
                ).apply {
                    setMargins(0, 32, 0, 32)
                }
            }
            "text" -> TextView(context).apply {
                text = component.text ?: ""
                textSize = 18f
                component.style?.let {
                    if (it == "error") setTextColor(Color.RED) else setTextColor(Color.DKGRAY)
                }
            }
            "input" -> EditText(context).apply {
                hint = component.text ?: ""
                textSize = 18f
                inputViews[component.id] = this
            }
            "button" -> Button(context).apply {
                text = component.text ?: ""
                component.action?.let { actionId ->
                    setOnClickListener {
                        var resolvedData = component.data
                        // Resolve data if it refers to an input field
                        if (resolvedData != null && inputViews.containsKey(resolvedData)) {
                            resolvedData = inputViews[resolvedData]?.text?.toString()
                        }
                        actionDispatcher(actionId, resolvedData)
                    }
                }
            }
            "icon_button" -> Button(context).apply {
                text = component.text ?: "Icon"
                component.action?.let { actionId ->
                    setOnClickListener {
                        var resolvedData = component.data
                        if (resolvedData != null && inputViews.containsKey(resolvedData)) {
                            resolvedData = inputViews[resolvedData]?.text?.toString()
                        }
                        actionDispatcher(actionId, resolvedData)
                    }
                }
            }
            "container", "row", "column" -> {
                val layout = LinearLayout(context).apply {
                    orientation = if (component.type == "row") LinearLayout.HORIZONTAL else LinearLayout.VERTICAL
                }
                component.children.forEach { child ->
                    buildComponent(child)?.let { layout.addView(it) }
                }
                layout
            }
            "spacer" -> View(context).apply {
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT, 
                    32
                )
            }
            "call_timer" -> TextView(context).apply {
                textSize = 18f
                text = "00:00"
                val startTime = System.currentTimeMillis()
                val handler = android.os.Handler(android.os.Looper.getMainLooper())
                val runnable = object : Runnable {
                    override fun run() {
                        val elapsed = System.currentTimeMillis() - startTime
                        val seconds = (elapsed / 1000) % 60
                        val minutes = (elapsed / (1000 * 60)) % 60
                        text = String.format("%02d:%02d", minutes, seconds)
                        handler.postDelayed(this, 1000)
                    }
                }
                handler.post(runnable)
                addOnAttachStateChangeListener(object : View.OnAttachStateChangeListener {
                    override fun onViewAttachedToWindow(v: View) {}
                    override fun onViewDetachedFromWindow(v: View) {
                        handler.removeCallbacks(runnable)
                    }
                })
            }
            "local_video" -> {
                val container = android.widget.FrameLayout(context)
                container.layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT,
                    600
                ).apply {
                    setMargins(0, 20, 0, 20)
                }
                
                val renderer = org.webrtc.SurfaceViewRenderer(context)
                renderer.tag = "local_video_renderer"
                renderer.layoutParams = android.widget.FrameLayout.LayoutParams(
                    android.widget.FrameLayout.LayoutParams.MATCH_PARENT,
                    android.widget.FrameLayout.LayoutParams.MATCH_PARENT
                )
                
                container.addView(renderer)
                container
            }
            "remote_video" -> {
                val container = android.widget.FrameLayout(context)
                container.layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT,
                    600
                ).apply {
                    setMargins(0, 20, 0, 20)
                }
                container.setBackgroundColor(android.graphics.Color.DKGRAY)
                
                val renderer = org.webrtc.SurfaceViewRenderer(context)
                renderer.tag = "remote_video_renderer"
                renderer.layoutParams = android.widget.FrameLayout.LayoutParams(
                    android.widget.FrameLayout.LayoutParams.MATCH_PARENT,
                    android.widget.FrameLayout.LayoutParams.MATCH_PARENT
                )
                
                container.addView(renderer)
                container
            }
            else -> null
        }
    }

    private fun buildActiveVideoCallLayout(schema: SduiSchema): View {
        val root = android.widget.FrameLayout(context)
        root.layoutParams = android.widget.FrameLayout.LayoutParams(
            android.widget.FrameLayout.LayoutParams.MATCH_PARENT,
            android.widget.FrameLayout.LayoutParams.MATCH_PARENT
        )
        root.setBackgroundColor(Color.BLACK)

        // Find components from schema safely
        val componentsList = schema.components ?: emptyList()
        val remoteVideoComp = componentsList.find { it.id == "remote_video" }
        val localVideoComp = componentsList.find { it.id == "local_video" }
        
        // Remote Video Fullscreen
        if (remoteVideoComp != null) {
            val remoteRenderer = org.webrtc.SurfaceViewRenderer(context)
            remoteRenderer.tag = "remote_video_renderer"
            remoteRenderer.layoutParams = android.widget.FrameLayout.LayoutParams(
                android.widget.FrameLayout.LayoutParams.MATCH_PARENT,
                android.widget.FrameLayout.LayoutParams.MATCH_PARENT
            )
            root.addView(remoteRenderer)
        }

        // Local Video PiP removed as per user request ("2jon camra viaw hoba na... full seen")

        // Top info (Peer Name + Timer)
        val infoContainer = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            gravity = android.view.Gravity.CENTER_HORIZONTAL
            layoutParams = android.widget.FrameLayout.LayoutParams(
                android.widget.FrameLayout.LayoutParams.MATCH_PARENT,
                android.widget.FrameLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                gravity = android.view.Gravity.TOP or android.view.Gravity.CENTER_HORIZONTAL
                setMargins(0, (50 * context.resources.displayMetrics.density).toInt(), 0, 0)
            }
        }
        
        val peerComp = componentsList.find { it.id == "peer_name" }
        val timerComp = componentsList.find { it.id == "call_timer" }
        
        if (peerComp != null) {
            val pView = buildComponent(peerComp) as? TextView
            pView?.setTextColor(Color.WHITE)
            pView?.setShadowLayer(4f, 0f, 2f, Color.BLACK)
            pView?.textSize = 24f
            pView?.let { infoContainer.addView(it) }
        }
        if (timerComp != null) {
            val tView = buildComponent(timerComp) as? TextView
            tView?.setTextColor(Color.WHITE)
            tView?.setShadowLayer(4f, 0f, 2f, Color.BLACK)
            tView?.let { infoContainer.addView(it) }
        }
        val visualizerComp = componentsList.find { it.type == "audio_visualizer" }
        if (visualizerComp != null) {
            val vView = buildComponent(visualizerComp)
            vView?.let { infoContainer.addView(it) }
        }
        
        root.addView(infoContainer)

        // Buttons Container at Bottom
        val buttonsContainer = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = android.view.Gravity.CENTER
            layoutParams = android.widget.FrameLayout.LayoutParams(
                android.widget.FrameLayout.LayoutParams.MATCH_PARENT,
                android.widget.FrameLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                gravity = android.view.Gravity.BOTTOM
                setMargins(0, 0, 0, (60 * context.resources.displayMetrics.density).toInt())
            }
        }

        val buttonIds = listOf("camera_switch", "camera_toggle", "mute_toggle", "switch_audio", "end_call")
        for (btnId in buttonIds) {
            val btnComp = componentsList.find { it.id == btnId }
            if (btnComp != null) {
                val btnView = buildComponent(btnComp)
                if (btnView != null) {
                    val lp = LinearLayout.LayoutParams(
                        0,
                        LinearLayout.LayoutParams.WRAP_CONTENT,
                        1f
                    ).apply {
                        setMargins(8, 0, 8, 0)
                    }
                    btnView.layoutParams = lp
                    buttonsContainer.addView(btnView)
                }
            }
        }
        
        root.addView(buttonsContainer)

        return root
    }
}
