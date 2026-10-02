package com.thinclient.model

data class SduiSchema(
    val schema_version: Int,
    val screen: String,
    val revision: Int,
    val title: String? = null,
    val components: List<SduiComponent> = emptyList()
)

data class SduiComponent(
    val id: String,
    val type: String,
    val text: String? = null,
    val action: String? = null,
    val style: String? = null,
    val data: String? = null,
    val children: List<SduiComponent> = emptyList()
)
