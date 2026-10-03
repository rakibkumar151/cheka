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

        schema.components.forEach { component ->
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
}
