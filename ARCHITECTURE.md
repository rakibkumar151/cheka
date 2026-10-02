# Architecture

## High-Level Topology

```text
                        ┌─────────────────────────┐
                        │       Android APK       │
                        │ Thin Runtime            │
                        └───────────┬─────────────┘
                                    │ TLS 1.3
                                    │ WebSocket / HTTPS
                                    ▼
                        ┌─────────────────────────┐
                        │       Rust Gateway      │
                        │ Axum/Tokio              │
                        └───────────┬─────────────┘
                                    │
               ┌────────────────────┼────────────────────┐
               ▼                    ▼                    ▼
      ┌────────────────┐  ┌──────────────────┐  ┌─────────────────┐
      │ SDUI Service   │  │ Call/Signaling   │  │ Presence        │
      └───────┬────────┘  └────────┬─────────┘  └────────┬────────┘
              │                    │                     │
              └────────────────────┼─────────────────────┘
                                   ▼
                         ┌────────────────────┐
                         │ Redis (ephemeral)  │
                         └─────────┬──────────┘
                                   │
                                   ▼
                         ┌────────────────────┐
                         │ libSQL / Turso     │
                         └────────────────────┘

                  WebRTC MEDIA PATH (SEPARATE)
                  ┌─────────────────────────┐
 Android A  ─────►│ STUN / ICE / TURN       │─────► Android B
                  └─────────────────────────┘
```

## 1. Android Thin Runtime
Responsible strictly for OS integration and local hardware capabilities:
- OS Permissions
- Camera and Microphone bridging
- WebRTC connection handling (PeerConnection)
- Local and remote surface rendering
- Executing strict Server-Driven UI (SDUI) schemas
- Dispatching user actions via WebSocket to Rust backend

## 2. Rust Gateway & Backend
Acts as the brain of the application:
- Axum for REST (Auth, initial bootstrap)
- Tokio-based WebSocket for bidirectional signaling
- Evaluates feature flags and pushes UI diffs/schemas
- Manages strict Call State Machine
- Delegates horizontal scaling messages to Redis (Pub/Sub)
- Persists durable state (Users, Call Logs, Config) to Turso/libSQL

## 3. WebRTC & Media
- Peer-to-Peer media streaming (audio/video).
- Signaling handled entirely by Rust WebSockets.
- Media traffic NEVER goes through the Rust API; uses direct P2P or TURN relays.

## 4. Horizontal Scalability
- Gateways are completely stateless.
- Session routing and real-time events synchronize through Redis.
- Allows massive scale to hit 100,000+ concurrent WebSockets.
