# Requirements Checklist

## 1. Product Scope (Phase 1)
- [ ] Sign in / session bootstrap
- [ ] See minimal current availability/presence
- [ ] Start 1:1 audio call
- [ ] Start 1:1 video call
- [ ] Receive incoming audio call
- [ ] Receive incoming video call
- [ ] Accept/Reject/End call
- [ ] Mute/unmute microphone
- [ ] Camera on/off, front/back switch
- [ ] Reconnect after transient network failure
- [ ] Show calling states, remote video, local preview, call duration
- [ ] Report call connection state

## 2. Server-Driven UI (SDUI)
- [ ] Server sends versioned UI schema (screen, components, actions)
- [ ] Client parses schema and renders components
- [ ] Client executes only whitelisted actions
- [ ] Client reports action results to server
- [ ] No arbitrary remote code execution (no Kotlin/Java scripts)

## 3. Thin-Client Bootstrap (Android)
- [ ] Networking, TLS, WebSocket/HTTPS clients
- [ ] Auth/session storage
- [ ] SDUI renderer and action whitelist
- [ ] OS permissions, Camera/Mic bridge, WebRTC engine

## 4. Rust Backend
- [ ] Modular architecture (Auth, WS, Signaling, SDUI, Calls, Turso, Redis)
- [ ] Tokio, Axum, rustls
- [ ] Redis for ephemeral realtime state
- [ ] libSQL/Turso for durable metadata
- [ ] Secrets restricted to backend `.env`

## 5. WebRTC & Signaling
- [ ] Explicit call state machine (IDLE -> ... -> CONNECTED -> ENDED)
- [ ] Authenticated WebSocket channel for signaling
- [ ] WebRTC direct path or TURN relay (no media proxying through Rust REST API)
- [ ] ICE candidates and SDP exchanged via WS

## 6. Security & Target
- [ ] No hardcoded secrets in Android APK
- [ ] All sensitive actions require server-side authorization
- [ ] Measurable load-test target: 100,000 concurrent authenticated sessions
- [ ] Horizontal scaling supported by stateless gateways
