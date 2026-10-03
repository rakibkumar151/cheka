use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SduiComponentType {
    Screen,
    Container,
    Text,
    Avatar,
    Image,
    Button,
    IconButton,
    Spacer,
    Row,
    Column,
    CallTimer,
    LocalVideo,
    RemoteVideo,
    MicState,
    CameraState,
    ConnectionState,
    Input,
    AudioVisualizer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SduiActionId {
    Navigate,
    #[serde(rename = "call.start_audio")]
    CallStartAudio,
    #[serde(rename = "call.start_video")]
    CallStartVideo,
    #[serde(rename = "call.accept")]
    CallAccept,
    #[serde(rename = "call.reject")]
    CallReject,
    #[serde(rename = "call.end")]
    CallEnd,
    #[serde(rename = "call.mute")]
    CallMute,
    #[serde(rename = "call.unmute")]
    CallUnmute,
    #[serde(rename = "call.camera_on")]
    CallCameraOn,
    #[serde(rename = "call.camera_off")]
    CallCameraOff,
    #[serde(rename = "call.camera_switch")]
    CallCameraSwitch,
    #[serde(rename = "call.switch_audio")]
    CallSwitchAudio,
    Retry,
    Logout,
    #[serde(rename = "copy_text")]
    CopyText,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SduiComponent {
    pub id: String,
    #[serde(rename = "type")]
    pub component_type: SduiComponentType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<SduiActionId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub children: Vec<SduiComponent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SduiSchema {
    pub schema_version: u32,
    pub screen: String,
    pub revision: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub components: Vec<SduiComponent>,
}

impl SduiSchema {
    pub fn validate(&self) -> bool {
        if self.schema_version != 1 {
            return false;
        }
        if self.screen.is_empty() {
            return false;
        }
        true
    }
}

use crate::presence::UserPresence;

pub fn build_home_screen(my_uid: &str, online_users: &[(String, UserPresence)], revision: u32) -> SduiSchema {
    let mut components = vec![
        SduiComponent {
            id: "lbl_my_uid".to_string(),
            component_type: SduiComponentType::Text,
            text: Some("My UID".to_string()),
            action: None, style: None, data: None, children: vec![],
        },
        SduiComponent {
            id: "my_uid_value".to_string(),
            component_type: SduiComponentType::Text,
            text: Some(my_uid.to_string()),
            action: None, style: None, data: None, children: vec![],
        },
        SduiComponent {
            id: "btn_copy_uid".to_string(),
            component_type: SduiComponentType::Button,
            text: Some("COPY UID".to_string()),
            action: Some(SduiActionId::CopyText),
            data: Some(my_uid.to_string()),
            style: None, children: vec![],
        },
        SduiComponent {
            id: "lbl_online_users".to_string(),
            component_type: SduiComponentType::Text,
            text: Some("ONLINE NOW".to_string()),
            action: None, style: None, data: None, children: vec![],
        }
    ];

    if online_users.is_empty() {
        components.push(SduiComponent {
            id: "lbl_no_users".to_string(),
            component_type: SduiComponentType::Text,
            text: Some("No other users online".to_string()),
            action: None, style: None, data: None, children: vec![],
        });
    } else {
        for (uid, presence) in online_users {
            // Add a row for each user
            components.push(SduiComponent {
                id: format!("row_{}", uid),
                component_type: SduiComponentType::Row,
                text: None, action: None, style: None, data: None,
                children: vec![
                    SduiComponent {
                        id: format!("lbl_{}", uid),
                        component_type: SduiComponentType::Text,
                        text: Some(format!("🟢 {}", uid)),
                        action: None, style: None, data: None, children: vec![],
                    },
                    SduiComponent {
                        id: format!("btn_audio_{}", uid),
                        component_type: SduiComponentType::Button,
                        text: Some("[AUDIO]".to_string()),
                        action: Some(SduiActionId::CallStartAudio),
                        data: Some(uid.clone()),
                        style: None, children: vec![],
                    },
                    SduiComponent {
                        id: format!("btn_video_{}", uid),
                        component_type: SduiComponentType::Button,
                        text: Some("[VIDEO]".to_string()),
                        action: Some(SduiActionId::CallStartVideo),
                        data: Some(uid.clone()),
                        style: None, children: vec![],
                    }
                ],
            });
        }
    }

    SduiSchema {
        schema_version: 1,
        screen: "debug_call".to_string(),
        revision,
        title: Some("Debug Call Test".to_string()),
        components,
    }
}

use std::sync::Arc;
use crate::AppState;

pub async fn push_sdui_to_local_users(state: &Arc<AppState>) {
    let online_users = state.presence.get_online_users().await;
    let local_users = state.gateway.routing.local_users.read().await;
    for (local_uid, (_, tx)) in local_users.iter() {
        let other_users: Vec<_> = online_users.iter().filter(|(uid, _)| uid != local_uid).cloned().collect();
        let schema = serde_json::to_value(build_home_screen(
            local_uid,
            &other_users,
            1,
        )).unwrap();

        let msg = crate::ws::WsMessage {
            msg_type: crate::ws::WsMessageType::SduiUpdate,
            request_id: uuid::Uuid::new_v4().to_string(),
            session_id: "presence_update".to_string(),
            call_id: None,
            seq: 1,
            payload: schema,
        };
        let _ = tx.send(msg).await;
    }
}

pub fn build_incoming_call_screen(
    caller_name: &str,
    caller_avatar: &str,
    call_id: &str,
    revision: u32,
) -> SduiSchema {
    SduiSchema {
        schema_version: 1,
        screen: "incoming_call".to_string(),
        revision,
        title: Some("Incoming video call".to_string()),
        components: vec![
            SduiComponent {
                id: "caller".to_string(),
                component_type: SduiComponentType::Avatar,
                data: Some(caller_avatar.to_string()),
                text: None,
                action: None,
                style: None,
                children: vec![],
            },
            SduiComponent {
                id: "name".to_string(),
                component_type: SduiComponentType::Text,
                text: Some(caller_name.to_string()),
                data: None,
                action: None,
                style: None,
                children: vec![],
            },
            SduiComponent {
                id: "accept".to_string(),
                component_type: SduiComponentType::Button,
                action: Some(SduiActionId::CallAccept),
                style: Some("primary".to_string()),
                text: Some("Accept".to_string()),
                data: Some(call_id.to_string()),
                children: vec![],
            },
            SduiComponent {
                id: "reject".to_string(),
                component_type: SduiComponentType::Button,
                action: Some(SduiActionId::CallReject),
                style: Some("destructive".to_string()),
                text: Some("Decline".to_string()),
                data: Some(call_id.to_string()),
                children: vec![],
            },
        ],
    }
}

pub fn build_active_audio_call_screen(
    peer_name: &str,
    is_muted: bool,
    call_id: &str,
    revision: u32,
) -> SduiSchema {
    SduiSchema {
        schema_version: 1,
        screen: "active_audio_call".to_string(),
        revision,
        title: Some("Active Call".to_string()),
        components: vec![
            SduiComponent {
                id: "peer_name".to_string(),
                component_type: SduiComponentType::Text,
                text: Some(peer_name.to_string()),
                data: None,
                action: None,
                style: None,
                children: vec![],
            },
            SduiComponent {
                id: "call_timer".to_string(),
                component_type: SduiComponentType::CallTimer,
                text: None,
                data: None,
                action: None,
                style: None,
                children: vec![],
            },
            SduiComponent {
                id: "audio_visualizer".to_string(),
                component_type: SduiComponentType::AudioVisualizer,
                text: None,
                data: None,
                action: None,
                style: None,
                children: vec![],
            },
            SduiComponent {
                id: "mute_toggle".to_string(),
                component_type: SduiComponentType::IconButton,
                action: Some(if is_muted {
                    SduiActionId::CallUnmute
                } else {
                    SduiActionId::CallMute
                }),
                text: Some(if is_muted { "Unmute".to_string() } else { "Mute".to_string() }),
                data: Some(call_id.to_string()),
                style: None,
                children: vec![],
            },
            SduiComponent {
                id: "switch_video".to_string(),
                component_type: SduiComponentType::IconButton,
                action: Some(SduiActionId::CallCameraOn),
                text: Some("Switch to Video".to_string()),
                data: Some(call_id.to_string()),
                style: None,
                children: vec![],
            },
            SduiComponent {
                id: "end_call".to_string(),
                component_type: SduiComponentType::IconButton,
                action: Some(SduiActionId::CallEnd),
                text: Some("End Call".to_string()),
                data: Some(call_id.to_string()),
                style: Some("destructive".to_string()),
                children: vec![],
            },
        ],
    }
}

pub fn build_bootstrap_screen(revision: u32) -> SduiSchema {
    SduiSchema {
        schema_version: 1,
        screen: "bootstrap".to_string(),
        revision,
        title: Some("Connecting...".to_string()),
        components: vec![SduiComponent {
            id: "loading".to_string(),
            component_type: SduiComponentType::ConnectionState,
            text: Some("Initializing...".to_string()),
            data: None,
            action: None,
            style: None,
            children: vec![],
        }],
    }
}

pub fn build_outgoing_call_screen(peer_name: &str, revision: u32) -> SduiSchema {
    SduiSchema {
        schema_version: 1,
        screen: "outgoing_call".to_string(),
        revision,
        title: Some("Calling...".to_string()),
        components: vec![
            SduiComponent {
                id: "name".to_string(),
                component_type: SduiComponentType::Text,
                text: Some(peer_name.to_string()),
                data: None,
                action: None,
                style: None,
                children: vec![],
            },
            SduiComponent {
                id: "end_call".to_string(),
                component_type: SduiComponentType::IconButton,
                action: Some(SduiActionId::CallEnd),
                style: Some("destructive".to_string()),
                text: None,
                data: None,
                children: vec![],
            },
        ],
    }
}

pub fn build_connecting_screen(revision: u32) -> SduiSchema {
    SduiSchema {
        schema_version: 1,
        screen: "connecting".to_string(),
        revision,
        title: Some("Connecting...".to_string()),
        components: vec![SduiComponent {
            id: "status".to_string(),
            component_type: SduiComponentType::ConnectionState,
            text: Some("Establishing connection...".to_string()),
            data: None,
            action: None,
            style: None,
            children: vec![],
        }],
    }
}

pub fn build_active_video_call_screen(
    peer_name: &str,
    is_muted: bool,
    camera_on: bool,
    call_id: &str,
    revision: u32,
) -> SduiSchema {
    SduiSchema {
        schema_version: 1,
        screen: "active_video_call".to_string(),
        revision,
        title: Some("Active Video Call".to_string()),
        components: vec![
            SduiComponent {
                id: "remote_video".to_string(),
                component_type: SduiComponentType::RemoteVideo,
                text: None,
                data: None,
                action: None,
                style: None,
                children: vec![],
            },
            SduiComponent {
                id: "local_video".to_string(),
                component_type: SduiComponentType::LocalVideo,
                text: None,
                data: None,
                action: None,
                style: None,
                children: vec![],
            },
            SduiComponent {
                id: "peer_name".to_string(),
                component_type: SduiComponentType::Text,
                text: Some(peer_name.to_string()),
                data: None,
                action: None,
                style: None,
                children: vec![],
            },
            SduiComponent {
                id: "call_timer".to_string(),
                component_type: SduiComponentType::CallTimer,
                text: None,
                data: None,
                action: None,
                style: None,
                children: vec![],
            },
            SduiComponent {
                id: "audio_visualizer".to_string(),
                component_type: SduiComponentType::AudioVisualizer,
                text: None,
                data: None,
                action: None,
                style: None,
                children: vec![],
            },
            SduiComponent {
                id: "mute_toggle".to_string(),
                component_type: SduiComponentType::IconButton,
                action: Some(if is_muted {
                    SduiActionId::CallUnmute
                } else {
                    SduiActionId::CallMute
                }),
                text: Some(if is_muted { "Unmute".to_string() } else { "Mute".to_string() }),
                data: Some(call_id.to_string()),
                style: None,
                children: vec![],
            },
            SduiComponent {
                id: "camera_toggle".to_string(),
                component_type: SduiComponentType::IconButton,
                action: Some(if camera_on {
                    SduiActionId::CallCameraOff
                } else {
                    SduiActionId::CallCameraOn
                }),
                text: Some(if camera_on { "Turn Camera Off".to_string() } else { "Turn Camera On".to_string() }),
                data: Some(call_id.to_string()),
                style: None,
                children: vec![],
            },
            SduiComponent {
                id: "camera_switch".to_string(),
                component_type: SduiComponentType::IconButton,
                action: Some(SduiActionId::CallCameraSwitch),
                text: Some("Flip Camera".to_string()),
                data: Some(call_id.to_string()),
                style: None,
                children: vec![],
            },
            SduiComponent {
                id: "switch_audio".to_string(),
                component_type: SduiComponentType::IconButton,
                action: Some(SduiActionId::CallSwitchAudio),
                text: Some("Switch to Audio".to_string()),
                data: Some(call_id.to_string()),
                style: None,
                children: vec![],
            },
            SduiComponent {
                id: "end_call".to_string(),
                component_type: SduiComponentType::IconButton,
                action: Some(SduiActionId::CallEnd),
                text: Some("End Call".to_string()),
                data: None,
                style: Some("destructive".to_string()),
                children: vec![],
            },
        ],
    }
}

pub fn build_call_ended_screen(revision: u32) -> SduiSchema {
    SduiSchema {
        schema_version: 1,
        screen: "call_ended".to_string(),
        revision,
        title: Some("Call Ended".to_string()),
        components: vec![
            SduiComponent {
                id: "status".to_string(),
                component_type: SduiComponentType::Text,
                text: Some("The call has ended.".to_string()),
                data: None,
                action: None,
                style: None,
                children: vec![],
            },
            SduiComponent {
                id: "close".to_string(),
                component_type: SduiComponentType::Button,
                action: Some(SduiActionId::Navigate),
                style: Some("primary".to_string()),
                text: Some("Close".to_string()),
                data: Some("home".to_string()),
                children: vec![],
            },
        ],
    }
}

pub fn build_call_error_screen(error_msg: &str, revision: u32) -> SduiSchema {
    SduiSchema {
        schema_version: 1,
        screen: "call_error".to_string(),
        revision,
        title: Some("Error".to_string()),
        components: vec![
            SduiComponent {
                id: "error_text".to_string(),
                component_type: SduiComponentType::Text,
                text: Some(error_msg.to_string()),
                data: None,
                action: None,
                style: Some("error".to_string()),
                children: vec![],
            },
            SduiComponent {
                id: "retry".to_string(),
                component_type: SduiComponentType::Button,
                action: Some(SduiActionId::Retry),
                style: Some("primary".to_string()),
                text: Some("Retry".to_string()),
                data: None,
                children: vec![],
            },
        ],
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_schema() {
        let schema = build_incoming_call_screen("Alice", "url", 1);
        assert!(schema.validate());
    }

    #[test]
    fn test_malformed_schema() {
        let schema = SduiSchema {
            schema_version: 2, // invalid
            screen: "test".to_string(),
            revision: 1,
            title: None,
            components: vec![],
        };
        assert!(!schema.validate());
    }
}
