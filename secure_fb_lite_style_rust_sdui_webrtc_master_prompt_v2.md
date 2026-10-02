# Secure FB-Lite-Style Thin Client + Rust Backend
## Master AI Build Prompt v2
### Scope: Native Android + Rust + libSQL/Turso + Server-Driven UI + 1:1 Audio/Video Calling

> **Purpose**
>
> Use this document as the single source of truth for building a lightweight, security-focused Android application whose product/UI behavior is controlled as much as practical by a Rust backend.
>
> The first release contains **ONLY 1:1 audio calling and 1:1 video calling**. Chat is explicitly out of scope until the calling phase is fully tested.
>
> The architecture is inspired by the publicly documented **thin-client / server-heavy approach of Facebook Lite**, but this project must not pretend to reproduce Meta's private implementation. Facebook Lite's published 2016 architecture used a very thin client, server-side product code, compressed UI trees, and a persistent TLS connection; the Android client still provided OS capabilities and rendering. See: https://engineering.fb.com/2016/03/09/android/how-we-built-facebook-lite-for-every-android-phone-and-network/
>
> **Security reality:** do NOT claim "100% impossible to leak". No software can honestly guarantee that. The acceptance target is **defense in depth + least privilege + no known critical/high findings + measured security tests + zero secrets in the APK**.

---

## 0. NON-NEGOTIABLE RULES

1. **Never hardcode secrets into Android.**
   - No Turso auth token in APK/source/resources.
   - No TURN static credential in APK.
   - No database credential in the client.
   - No private signing key in Git.
   - No production secret in logs.

2. **The Turso credential previously shared in chat is treated as COMPROMISED.**
   - Revoke/rotate it before any real deployment.
   - Use placeholders in all generated files:
     - `TURSO_DATABASE_URL=libsql://...`
     - `TURSO_AUTH_TOKEN=<REDACTED>`
   - Never echo the actual token into code or documentation.

3. **The Android app is a thin runtime, not a normal feature-heavy client.**
   - Product UI definitions, screen schemas, labels, layout parameters, feature flags, navigation decisions, call-state UI, and server-controlled actions should come from the backend as much as practical.
   - A minimal bootstrap shell, networking, secure credential handling, OS permission bridge, camera/microphone bridge, and WebRTC media renderer must remain client-side because the device OS/hardware requires them.

4. **The calling media path is WebRTC.**
   - The Rust backend controls authentication, authorization, call sessions, signaling, policy, TURN credential issuance, presence, and state.
   - Audio/video packets must NOT pass through the ordinary Rust API layer.
   - Direct WebRTC may be used when possible; TURN relay is used when required.
   - Do not implement a custom RTP/crypto stack unless there is a documented, tested requirement.

5. **No security-by-obscurity.**
   - Do not hide endpoints as a substitute for authentication.
   - Every sensitive action requires server-side authorization.
   - Assume the APK can be reverse engineered.

6. **Do not promise 100,000 concurrent users without load testing.**
   - Treat 100,000 concurrent connected sessions as a measurable engineering target.
   - Produce load-test results, bottleneck data, and scaling decisions.
   - A successful design must scale horizontally.

7. **Never mark an item "DONE" unless it was actually implemented and tested.**
   - Use `DONE`, `PARTIAL`, `BLOCKED`, `NOT IMPLEMENTED`, or `NOT TESTED`.

---

# 1. PRODUCT SCOPE

## Phase 1 ONLY

### User capabilities
- Sign in / session bootstrap
- See minimal current availability/presence
- Start 1:1 audio call
- Start 1:1 video call
- Receive incoming audio call
- Receive incoming video call
- Accept
- Reject
- End
- Mute/unmute microphone
- Camera on/off
- Switch front/back camera
- Reconnect after transient network failure
- Show calling states
- Show remote video
- Show local preview
- Show call duration
- Report call connection state

### Explicitly NOT IN PHASE 1
- Chat
- Groups
- Stories
- Feed
- Comments
- Search
- Payments
- Ads
- Public profiles
- File sharing
- Group calling

Do not add extra features.

---

# 2. TARGET ARCHITECTURE

```text
                        ┌─────────────────────────┐
                        │       Android APK       │
                        │                         │
                        │ Thin Runtime            │
                        │ - Bootstrap shell        │
                        │ - SDUI renderer          │
                        │ - Action dispatcher      │
                        │ - Secure token storage   │
                        │ - WS/TLS client          │
                        │ - Camera/Mic bridge      │
                        │ - WebRTC media engine    │
                        │ - Local call renderer    │
                        └───────────┬─────────────┘
                                    │ TLS 1.3
                                    │ WebSocket / HTTPS
                                    ▼
                        ┌─────────────────────────┐
                        │       Rust Gateway      │
                        │ Axum/Tokio              │
                        │ Rate limits             │
                        │ Auth middleware          │
                        │ Request IDs             │
                        └───────────┬─────────────┘
                                    │
               ┌────────────────────┼────────────────────┐
               ▼                    ▼                    ▼
      ┌────────────────┐  ┌──────────────────┐  ┌─────────────────┐
      │ SDUI Service   │  │ Call/Signaling   │  │ Presence        │
      │ Screen schemas │  │ Call state       │  │ Online state    │
      │ Versions       │  │ SDP/ICE relay    │  │ Heartbeats      │
      │ Diffs          │  │ Policy           │  │ TTL             │
      └───────┬────────┘  └────────┬─────────┘  └────────┬────────┘
              │                    │                     │
              └────────────────────┼─────────────────────┘
                                   ▼
                         ┌────────────────────┐
                         │ Redis (ephemeral)  │
                         │ sessions/presence  │
                         │ pub/sub or streams  │
                         │ rate limiting       │
                         └─────────┬──────────┘
                                   │
                                   ▼
                         ┌────────────────────┐
                         │ libSQL / Turso     │
                         │ durable state      │
                         │ users/calls/config │
                         └────────────────────┘

                  WebRTC MEDIA PATH (SEPARATE)
                  ┌─────────────────────────┐
 Android A  ─────►│ STUN / ICE / TURN       │─────► Android B
                  │ encrypted media path    │
                  └─────────────────────────┘
```

---

# 3. WHAT "MAXIMUM SERVER-DRIVEN UI" MEANS

The client must NOT contain a separate hardcoded implementation for every product screen.

The server sends a versioned UI schema such as:

```json
{
  "schema": 1,
  "screen": "incoming_call",
  "revision": 42,
  "title": "Incoming video call",
  "components": [
    {
      "id": "caller",
      "type": "avatar",
      "data": "$call.caller.avatar"
    },
    {
      "id": "name",
      "type": "text",
      "text": "$call.caller.display_name"
    },
    {
      "id": "accept",
      "type": "button",
      "action": "call.accept",
      "style": "primary"
    },
    {
      "id": "reject",
      "type": "button",
      "action": "call.reject",
      "style": "destructive"
    }
  ]
}
```

### Client responsibilities
- Parse schema
- Validate schema version
- Validate allowed component/action types
- Render components
- Execute ONLY whitelisted actions
- Report action result to server
- Cache a safe last-known-good schema
- Reject unknown/unsafe actions
- Never execute arbitrary code from the server

### Server responsibilities
- Decide which screen/state is active
- Send UI schema
- Send UI data
- Send feature flags
- Send button visibility/availability
- Send navigation decisions
- Send server-controlled strings
- Send call state
- Send timed refresh/update events
- Roll out UI revisions in realtime

### Critical security rule
Server-driven UI must be **data-driven**, not **remote code execution**.

Never send:
- Kotlin code
- Java bytecode
- native libraries
- shell commands
- arbitrary scripts
- arbitrary reflection targets

---

# 4. THIN-CLIENT BOOTSTRAP

The Android application may have only these permanent local capabilities:

```text
Bootstrap
Networking
TLS
Authentication/session storage
SDUI renderer
Action whitelist
Android permission bridge
Camera bridge
Microphone bridge
WebRTC engine
Local media renderer
Crash-safe recovery
```

Everything else should be delivered/configured by the Rust server.

### Bootstrap UI
A tiny emergency screen may be hardcoded for:
- first launch
- update/config fetch failure
- authentication failure
- network unavailable
- unsupported schema version
- fatal call error

This is allowed because a bootstrap path must exist before the server can deliver UI.

---

# 5. RUST BACKEND

Recommended stack:

```text
Rust
Tokio
Axum
serde / serde_json
sqlx OR libsql crate
Redis client
rustls
tracing
uuid
time
argon2 (only if password authentication is used)
jsonwebtoken or a well-audited session mechanism
```

For high-concurrency WebSocket handling, use async I/O and bounded resource usage. The current `tokio-websockets` documentation describes a Tokio-native, performance-oriented WebSocket implementation; choose the exact WebSocket crate only after benchmarking the selected stack.

### Rust service boundaries

Keep the first deployment modular even if it is initially one binary:

```text
rust-server/
  src/
    main.rs
    config/
    http/
    ws/
    auth/
    users/
    presence/
    calls/
    signaling/
    sdui/
    policy/
    turn/
    storage/
    telemetry/
    rate_limit/
    errors/
```

Do NOT split into dozens of microservices on day one unless profiling proves it is necessary.

Start with a modular monolith + Redis + Turso/libSQL, then extract bottlenecks.

---

# 6. TURSO / LIBSQL

Use credentials ONLY on the backend:

```env
TURSO_DATABASE_URL=libsql://YOUR_DATABASE.turso.io
TURSO_AUTH_TOKEN=YOUR_ROTATED_TOKEN
```

Do not include `.env` in Git.

Turso's Rust documentation supports the `libsql` crate and remote/embedded-replica patterns. For this project, start with a backend-controlled remote connection and only add embedded replicas after measuring whether they solve a real read-scaling or latency problem.

### Database tables

Minimal Phase 1 schema:

```sql
users
sessions
devices
call_sessions
call_participants
call_events
sdui_documents
sdui_revisions
feature_flags
audit_events
```

### Important rule

Do NOT store:
- raw access tokens
- TURN secrets
- private encryption keys
- microphone data
- raw video
- message plaintext (chat is out of scope anyway)

Store only the minimum durable metadata required.

---

# 7. CALL SESSION MODEL

Use an explicit state machine:

```text
IDLE
  ↓
INITIATING
  ↓
RINGING
  ↓
ACCEPTED
  ↓
NEGOTIATING
  ↓
CONNECTING
  ↓
CONNECTED
  ├── RECONNECTING
  │      ↓
  │   CONNECTED
  │
  └──────────────► ENDING
                       ↓
                     ENDED
```

Terminal states:
- ENDED
- REJECTED
- MISSED
- FAILED
- CANCELLED

Every transition must be validated server-side.

Example:

```text
RINGING → CONNECTED
```

must NOT be accepted directly unless the server has recorded:
- caller session exists
- callee accepted
- call participants are authorized
- signaling session exists
- call has not already ended

---

# 8. FULL AUDIO CALL FLOW

## A. Start

Android A:

```http
POST /v1/calls
Authorization: Bearer <access_token>
Content-Type: application/json

{
  "callee_id": "USER_B",
  "kind": "audio"
}
```

Rust:

1. Authenticate token
2. Load caller session
3. Authorize caller
4. Verify callee
5. Check block/privacy policy
6. Apply rate limit
7. Create `call_id`
8. Create call session
9. Publish incoming-call event
10. Deliver SDUI state to caller
11. Notify callee via WebSocket/push fallback

Response:

```json
{
  "call_id": "CALL_ID",
  "state": "RINGING",
  "ui_revision": 42
}
```

## B. Callee receives

Server event:

```json
{
  "type": "call.incoming",
  "call_id": "CALL_ID",
  "kind": "audio",
  "caller": {
    "id": "USER_A",
    "display_name": "Caller"
  }
}
```

Client asks SDUI renderer to show the server-defined incoming-call screen.

## C. Accept

```http
POST /v1/calls/CALL_ID/accept
```

Server:
- validates session
- validates participant
- transitions state `RINGING → ACCEPTED`
- creates/refreshes signaling session
- issues short-lived TURN credentials if required
- notifies caller

## D. WebRTC negotiation

Caller:
1. Obtain microphone permission.
2. Capture audio track.
3. Create PeerConnection.
4. Gather ICE candidates.
5. Create offer.
6. Send offer via signaling WebSocket.

Callee:
1. Obtain microphone permission.
2. Create PeerConnection.
3. Set remote offer.
4. Create answer.
5. Send answer via signaling.

Both:
- exchange ICE candidates through authenticated signaling
- establish DTLS/SRTP
- report connection state

## E. Connected

Server event:

```json
{
  "type": "call.state",
  "call_id": "CALL_ID",
  "state": "CONNECTED",
  "revision": 43
}
```

Client renders active call UI from SDUI while the WebRTC media view remains a native capability/renderer.

## F. End

Either side:

```http
POST /v1/calls/CALL_ID/end
```

Server:
- authenticates
- authorizes
- transitions to `ENDING`
- notifies peer
- records final state
- closes signaling resources
- expires short-lived call credentials

---

# 9. FULL VIDEO CALL FLOW

Everything above applies, plus:

## Local device
- request camera + microphone permissions
- acquire front-facing camera
- add audio and video tracks
- attach local preview

## Remote device
- receive remote tracks
- render remote video
- show fallback if video is unavailable

## Controls
Server-defined UI action identifiers:

```text
call.mute
call.unmute
call.camera_enable
call.camera_disable
call.camera_switch
call.end
```

The client maps ONLY these known action IDs to native operations.

Example:

```text
call.camera_disable
    ↓
validated action
    ↓
WebRTC video track enabled = false
    ↓
send state update
    ↓
server broadcasts authoritative call state
```

The server does not directly control the hardware.

---

# 10. SIGNALING PROTOCOL

Use one authenticated WebSocket channel per active app session:

```text
wss://api.example.com/v1/ws
```

Message envelope:

```json
{
  "type": "call.offer",
  "request_id": "UUID",
  "session_id": "SESSION",
  "call_id": "CALL_ID",
  "seq": 101,
  "payload": {}
}
```

Message types:

```text
session.ready
session.refresh
call.incoming
call.accepted
call.rejected
call.offer
call.answer
call.ice
call.state
call.end
call.error
sdui.update
feature_flags.update
presence.update
```

### Validation
Every message must validate:
- authenticated session
- call ownership
- participant membership
- message type allowlist
- schema version
- sequence number
- payload size
- rate limits
- expiration

Never trust `user_id` supplied inside a client payload when it can be derived from the authenticated session.

---

# 11. REALTIME SERVER-DRIVEN UI

The goal is:

```text
Rust config change
       ↓
new revision
       ↓
Redis pub/sub or stream
       ↓
all relevant gateway instances
       ↓
connected clients
       ↓
UI schema update
       ↓
renderer updates immediately
```

Example:

```json
{
  "type": "sdui.update",
  "screen": "active_call",
  "revision": 58,
  "patch": [
    {
      "op": "replace",
      "path": "/components/end_call/label",
      "value": "End"
    }
  ]
}
```

### Consistency rules

- Every document has a monotonically increasing revision.
- Client applies only newer revisions.
- Out-of-order updates are ignored.
- If a gap is detected, client requests a full document.
- Server stores last-known-good revision.
- Client falls back to last-known-good document on parse failure.
- No arbitrary code execution.

---

# 12. PERFORMANCE / 100,000 CONCURRENT USER TARGET

Do NOT claim this target is achieved until measured.

## Target definition

Minimum Phase 1 target:

```text
100,000 simultaneously connected authenticated sessions
```

This means WebSocket/application sessions, NOT 100,000 simultaneous video streams through one machine.

Calls have a separate media capacity problem.

### Required architecture for horizontal scale

```text
                Internet
                   │
            Load Balancer
                   │
      ┌────────────┼────────────┐
      ▼            ▼            ▼
 Gateway 1     Gateway 2     Gateway N
      │            │            │
      └────────────┼────────────┘
                   │
                Redis
                   │
                Turso
```

### Stateless gateway requirements
The Rust gateway should be as stateless as practical:
- JWT/session validation
- request handling
- WS connection management
- call signaling
- SDUI delivery
- no sticky-memory dependency for durable state

Connection-local state can remain in RAM, but cross-node communication must use shared infrastructure.

### Important limits
Set explicit bounds:
- max WebSocket frame size
- max concurrent streams per process
- max in-flight requests
- max pending signaling messages
- max payload size
- max call duration if a product limit is needed
- per-user call initiation limits

### Backpressure
Never allow unbounded queues.

Use:
- bounded channels
- timeouts
- cancellation
- shed load under pressure
- graceful shutdown
- circuit breakers where appropriate

---

# 13. MEDIA SCALING

For 1:1 calls:
- prefer direct WebRTC path when possible
- TURN relay when direct path fails
- do not proxy media through the Rust REST/API service
- TURN capacity must be load-tested separately

For large group calling later, use an SFU architecture rather than trying to scale 1:1 relay logic into a group system.

---

# 14. LOCAL / PRIVATE NETWORK TESTING

First test with two real Android devices on the same private Wi-Fi.

Example:

```text
Laptop:
192.168.1.10
Rust API:
0.0.0.0:8080

Phone A:
192.168.1.20

Phone B:
192.168.1.21
```

Android debug client connects to:

```text
http://192.168.1.10:8080
```

For production, use HTTPS/WSS. Do not ship cleartext HTTP.

### Recommended local test sequence

1. Start Rust backend bound to LAN interface.
2. Start Redis.
3. Start local/libSQL or test Turso connection.
4. Register/login two test users.
5. Connect both WebSocket sessions.
6. Verify presence.
7. Start audio call.
8. Accept.
9. Verify offer/answer/ICE.
10. Verify two-way audio.
11. End call.
12. Repeat for video.
13. Disconnect Wi-Fi temporarily and verify reconnection behavior.
14. Kill one app and verify remote end state.
15. Repeat with TURN disabled/enabled as appropriate.

---

# 15. API CONTRACT

Required endpoints:

```text
POST   /v1/auth/login
POST   /v1/auth/refresh
POST   /v1/auth/logout

GET    /v1/bootstrap
GET    /v1/sdui/screens/:screen

POST   /v1/calls
POST   /v1/calls/:id/accept
POST   /v1/calls/:id/reject
POST   /v1/calls/:id/end
GET    /v1/calls/:id

POST   /v1/calls/:id/turn-credentials
GET    /v1/health
GET    /v1/ready

WS     /v1/ws
```

Do not expose the database directly to Android.

---

# 16. AUTHENTICATION + AUTHORIZATION

Recommended:
- short-lived access token
- refresh token rotation
- server-side session revocation
- device/session list
- CSRF protection where cookie auth is used
- strict CORS only when web clients are present
- rate limiting

Authorization checks must answer:

```text
Can this authenticated user:
- access this call?
- accept this call?
- reject this call?
- end this call?
- send signaling for this call?
- fetch this SDUI document?
- update this feature flag?
```

Never authorize based only on a client-provided ID.

OWASP Mobile Security guidance explicitly calls for authentication/authorization on app components and protection against missing/incorrect authorization.

---

# 17. DATA MINIMIZATION

Store as little sensitive data as possible.

### Durable database
Keep:
- opaque user IDs
- minimal account metadata
- session metadata required for revocation
- call timestamps
- call status
- minimal audit data
- SDUI configuration revisions

Avoid storing:
- raw audio
- raw video
- access tokens
- refresh tokens in plaintext
- TURN long-lived credentials
- private keys
- unnecessary device identifiers

---

# 18. ANDROID SECURITY

Use:
- Android Keystore for app cryptographic keys where appropriate
- Encrypted local storage when local secrets are unavoidable
- internal private app storage
- network security configuration
- HTTPS/WSS
- certificate validation
- release signing
- R8/minification/obfuscation as appropriate
- no debug logging in release
- no secret values in resources or BuildConfig

OWASP guidance specifically identifies hardcoded cryptographic keys and sensitive data leakage from local storage/logs as security issues.

---

# 19. LOGGING

Safe log example:

```text
request_id=...
user_id_hash=...
call_id=...
event=call_accept
latency_ms=...
status=success
```

Never log:
- Authorization header
- access token
- refresh token
- password
- OTP
- TURN password
- SDP if privacy requirements prohibit it
- ICE candidate contents unless explicitly classified and protected
- raw media

Production logs must also have retention limits and access control.

---

# 20. RATE LIMITING

Examples:

```text
login attempts / IP
login attempts / account
call starts / user / minute
signaling messages / connection / second
SDUI fetches / session / minute
TURN credential requests / user / minute
```

Use Redis-backed rate limiting so limits work across multiple Rust instances.

---

# 21. FAILURE HANDLING

The server must handle:
- duplicate call start
- double accept
- accept after reject
- end after end
- stale call
- stale WebSocket
- reconnect
- duplicate signaling
- out-of-order signaling
- unauthorized signaling
- malformed SDP envelope
- oversized payload
- TURN credential expiry
- Redis unavailable
- Turso unavailable
- gateway overload

Do not panic on untrusted network input.

Return structured errors:

```json
{
  "error": {
    "code": "CALL_NOT_ACTIVE",
    "message": "Call is no longer active",
    "request_id": "UUID"
  }
}
```

Do not expose stack traces to clients.

---

# 22. SDUI SCHEMA SAFETY

Allowed component types should be an enum:

```text
screen
container
text
avatar
image
button
icon_button
spacer
row
column
call_timer
local_video
remote_video
mic_state
camera_state
connection_state
```

Allowed action IDs:

```text
navigate
call.start_audio
call.start_video
call.accept
call.reject
call.end
call.mute
call.unmute
call.camera_on
call.camera_off
call.camera_switch
retry
logout
```

Anything else:
- reject
- log security event
- keep last-known-good UI
- do not execute

---

# 23. VERSIONING / REALTIME UPDATES

Every SDUI payload must include:

```json
{
  "schema_version": 1,
  "revision": 123
}
```

Every app session reports:

```text
client_version
schema_version
device_api_level
renderer_capabilities
webrtc_capabilities
```

Server selects compatible UI.

### Emergency rollback

Server must support:

```text
revision 105 → broken
rollback → revision 104
```

Clients must receive the rollback without requiring an APK update.

This is the key benefit of the server-driven approach.

---

# 24. WHAT REMAINS CLIENT-SIDE BY NECESSITY

Do NOT force these onto the server:

```text
Camera capture
Microphone capture
Android permission prompt
Surface rendering
Video frame rendering
Audio playback
WebRTC PeerConnection
Android lifecycle
Foreground/background OS integration
Notification channel registration
```

These are device capabilities.

The goal is **maximum server-driven product behavior**, not pretending a server can physically operate device hardware.

---

# 25. APK SIZE STRATEGY

Use a tiny bootstrap/client runtime.

Avoid bundling:
- product JSON for every screen
- large image catalogs
- unused SDKs
- chat feature code
- unnecessary fonts/assets
- analytics libraries that are not required
- multiple duplicate networking libraries

Use:
- resource shrinking
- code shrinking where safe
- ABI splits / App Bundle
- lazy loading where possible
- server-delivered SDUI resources with caching

The public Facebook Lite architecture specifically focused on keeping the APK very small and shifting product code/resources server-side.

Do NOT set an arbitrary APK-size promise without measuring release builds.

---

# 26. TEST MATRIX

## Unit tests
- auth validation
- authorization policies
- call state transitions
- SDUI parser
- SDUI action allowlist
- revision ordering
- request validation
- rate limiting
- TURN credential expiration logic

## Integration tests
- login → bootstrap
- WS connect → authenticated session
- call start → incoming event
- accept → negotiation
- reject
- end
- reconnect
- stale session
- duplicate events
- Redis coordination
- Turso persistence

## Android tests
- permission denied
- permission granted
- camera unavailable
- microphone unavailable
- background/foreground
- orientation change
- network transition
- renderer receives new revision
- invalid SDUI schema fallback
- call UI state synchronization

## WebRTC tests
- audio-to-audio
- video-to-video
- camera switch
- mute/unmute
- remote track loss
- ICE failure
- reconnect
- TURN path
- packet loss simulation
- bandwidth limitation

## Security tests
- unauthorized call accept
- unauthorized call end
- cross-user call ID access
- replayed request
- expired token
- revoked token
- malformed WebSocket payload
- oversized WebSocket frame
- rate-limit bypass attempts
- IDOR testing
- secret scanning
- dependency audit
- debug logging scan
- APK string/resources secret scan
- backup/storage inspection
- TLS verification

---

# 27. LOAD TESTING

Build realistic tests, not just HTTP request spam.

Scenarios:

### Scenario A — 100k idle authenticated WebSockets
Measure:
- CPU
- memory
- open connections
- p95/p99 heartbeat latency
- reconnect rate

### Scenario B — high signaling activity
Measure:
- offer/answer latency
- ICE message latency
- Redis pub/sub latency
- dropped messages
- event-loop saturation

### Scenario C — call setup burst
Simulate large bursts of call starts/accepts.

### Scenario D — mixed workload
Example:
- 100k connected sessions
- fraction active in calls
- API traffic
- presence heartbeats
- SDUI updates
- TURN credential requests

### Do not count media streams as ordinary API connections.

For media, separately test TURN bandwidth and CPU/network limits.

### Required report
For every load test record:
- test name
- date/time
- build commit
- instance size
- number of nodes
- connections
- request/message rate
- CPU
- RAM
- network
- p50
- p95
- p99
- error rate
- reconnect rate
- bottleneck
- next scaling action

---

# 28. OBSERVABILITY

Every request/event should have:
- request_id
- trace_id
- user/session correlation ID (privacy-safe)
- call_id where applicable

Metrics:
```text
http_requests_total
http_request_duration
ws_connections
ws_disconnects
ws_messages_total
call_starts
call_accepts
call_rejects
call_failures
call_connected
call_duration
turn_credentials_issued
redis_latency
db_latency
sdui_revision_pushes
sdui_parse_failures
```

Tracing should not capture secrets or raw sensitive payloads.

---

# 29. BACKUP / DISASTER / RECOVERY

Define:
- database backup policy
- restore procedure
- secret rotation procedure
- Redis restart behavior
- gateway restart behavior
- client reconnect behavior
- configuration rollback

Test restore, do not merely document it.

---

# 30. DEPLOYMENT PHASES

## Phase 0 — local
```text
Android A
Android B
Laptop Rust server
Redis
local network
```

## Phase 1 — private test environment
```text
1+ Rust instances
Redis
Turso
TURN
TLS
test domain
```

## Phase 2 — controlled load test
Gradually increase:
```text
1k
5k
10k
25k
50k
100k
```

Do not jump directly to 100k without observing resource usage.

## Phase 3 — production
- multiple Rust gateway instances
- load balancer
- Redis HA strategy
- Turso production database
- TURN fleet
- monitoring
- alerting
- backups
- secret manager

---

# 31. RECOMMENDED PROJECT TREE

```text
secure-call-app/
├── android/
│   ├── app/
│   └── renderer/
├── server/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── config/
│       ├── http/
│       ├── ws/
│       ├── auth/
│       ├── calls/
│       ├── signaling/
│       ├── sdui/
│       ├── presence/
│       ├── turn/
│       ├── storage/
│       ├── rate_limit/
│       └── telemetry/
├── migrations/
├── loadtest/
├── security/
├── docs/
└── .env.example
```

---

# 32. AI IMPLEMENTATION INSTRUCTIONS

You are the coding agent.

### Step 1 — inspect
Read this entire specification before writing code.

### Step 2 — inventory
Create:
```text
REQUIREMENTS.md
ARCHITECTURE.md
THREAT_MODEL.md
TEST_PLAN.md
```

### Step 3 — implement backend
Implement secure Rust server first.

### Step 4 — implement thin Android runtime
Implement:
- bootstrap
- SDUI renderer
- action dispatcher
- auth session
- WebSocket
- WebRTC bridge

### Step 5 — integrate
Wire:
```text
Android → Rust → Redis/Turso
Android ↔ Rust signaling
Android ↔ WebRTC media
```

### Step 6 — local tests
Use two real devices on a private LAN.

### Step 7 — bug fixing
Every failed test must become:
```text
test → failure → diagnosis → fix → retest
```

### Step 8 — security review
Run code/dependency/secret/security checks.

### Step 9 — build
Produce a release/debug APK only after tests pass enough for the selected stage.

### Step 10 — report
Generate a final implementation report using the template below.

---

# 33. FINAL IMPLEMENTATION REPORT TEMPLATE

Create:

```text
IMPLEMENTATION_REPORT.md
```

Use exactly this structure:

```md
# Implementation Report

## 1. Build Information
- Commit:
- Rust version:
- Android Gradle Plugin:
- Android SDK:
- Build type:
- APK size:

## 2. Implemented Files
| File | Status | Description |
|------|--------|-------------|

## 3. Backend
- Axum:
- WebSocket:
- Auth:
- Authorization:
- SDUI:
- Call state machine:
- Signaling:
- Redis:
- Turso/libSQL:
- TURN credentials:
- Rate limiting:
- Logging:

## 4. Android
- Bootstrap:
- SDUI renderer:
- Action dispatcher:
- Keystore:
- Permissions:
- WebRTC:
- Audio:
- Video:
- Reconnection:

## 5. API Tests
| Test | Result | Evidence |
|------|--------|----------|

## 6. WebRTC Tests
| Test | Result | Evidence |
|------|--------|----------|

## 7. Security Tests
| Test | Result | Finding |
|------|--------|---------|

## 8. Load Tests
| Connections | p50 | p95 | p99 | Error Rate | CPU | RAM |
|-------------|-----|-----|-----|------------|-----|-----|

## 9. Bugs Found
| Bug | Severity | Root Cause | Fix | Retest |
|-----|----------|------------|-----|--------|

## 10. Known Limitations
- ...

## 11. NOT IMPLEMENTED
- ...

## 12. NOT TESTED
- ...

## 13. Security Conclusion
Do not say "100% secure".
State exactly which security controls were verified and which remain unverified.
```

---

# 34. DEFINITION OF DONE

Phase 1 is considered complete only when:

```text
[ ] Android thin runtime builds
[ ] No production secrets in APK
[ ] Backend builds in release mode
[ ] Rust tests pass
[ ] Android tests pass
[ ] SDUI schema validation works
[ ] Server can push a UI revision in realtime
[ ] Client applies newer revision
[ ] Client rejects invalid revision
[ ] Audio call starts
[ ] Audio call accepts
[ ] Two-way audio confirmed
[ ] Video call starts
[ ] Video call accepts
[ ] Two-way video confirmed
[ ] Camera switch works
[ ] Mute/unmute works
[ ] End call works
[ ] Reconnect tested
[ ] Unauthorized call action rejected
[ ] Secret scan passes
[ ] Dependency audit reviewed
[ ] TLS verified
[ ] Rate limiting verified
[ ] Redis multi-instance behavior tested
[ ] Turso access verified
[ ] Local two-device test passed
[ ] Load tests completed
[ ] 100k target status reported honestly
[ ] Final implementation report generated
```

---

# 35. IMPORTANT HONESTY RULE FOR THE AI

Never write:

- "100% secure"
- "zero chance of data leak"
- "100,000 users definitely supported"
- "production ready"

unless a documented test and evidence actually supports the specific claim.

Instead write:

- "No critical/high findings observed in the executed test set."
- "100k concurrent-session target passed/failed under the stated test environment."
- "TURN capacity not yet validated."
- "Production security audit not completed."

---

# 36. SECURITY THREAT MODEL

Threat actors:
- malicious client
- stolen session
- compromised device
- malicious user
- replay attacker
- API scraper
- malformed WebSocket client
- database credential thief
- misconfigured storage
- operator mistake
- dependency vulnerability
- leaked build artifact
- log/backup exposure

Primary protections:
```text
TLS
Authentication
Authorization
Short-lived credentials
Token rotation
Least privilege
Rate limiting
Input validation
Bounded queues
Private storage
Keystore
Secret manager
No hardcoded secrets
Minimal data retention
Audit logging
Dependency scanning
Load testing
Fail-safe SDUI parser
Call-state authorization
Short-lived TURN credentials
```

---

# 37. DATA FLOW — FINAL

```text
                    CONTROL PLANE

Android
  │
  ├──── HTTPS ─────► Rust Gateway ─────► Turso/libSQL
  │                       │
  │                       ├────────────► Redis
  │                       │
  │                       └────────────► SDUI / Call State
  │
  └──── WSS ─────────► Rust Signaling
                           │
                           └──── offer/answer/ICE


                    MEDIA PLANE

Android A
   │
   │ camera + microphone
   ▼
WebRTC
   │
   ├──────── direct path ─────────► Android B
   │
   └──────── TURN relay ──────────► Android B
```

**Never merge the ordinary API/database path with the raw media path.**

---

# 38. FINAL DESIGN PRINCIPLE

The desired result is:

```text
SMALL APK
   +
THIN CLIENT
   +
MAXIMUM SERVER-DRIVEN PRODUCT LOGIC
   +
RUST AS PRIMARY BACKEND
   +
REALTIME WEBSOCKET CONTROL
   +
WEBRTC FOR AUDIO/VIDEO
   +
REDIS FOR EPHEMERAL REALTIME STATE
   +
TURSO/LIBSQL FOR DURABLE DATA
   +
PRIVATE-BY-DEFAULT STORAGE
   +
NO SECRETS IN APK
   +
HORIZONTAL SCALING
   +
MEASURED LOAD TESTS
```

Do not copy Meta's private implementation or claim exact compatibility. Implement the same *class of architecture*: a small client runtime that receives server-controlled product/UI state while retaining only the native device capabilities required to execute it.

---

# 39. SOURCES TO CONSULT

1. Meta Engineering — Facebook Lite architecture:
   https://engineering.fb.com/2016/03/09/android/how-we-built-facebook-lite-for-every-android-phone-and-network/

2. Turso — Rust/libSQL:
   https://docs.turso.tech/sdk/rust

3. OWASP Mobile Application Security Verification Standard:
   https://mas.owasp.org/MASVS/

4. OWASP Android KeyStore guidance:
   https://mas.owasp.org/MASTG/knowledge/android/MASVS-STORAGE/MASTG-KNOW-0043/

5. OWASP Android storage/security testing:
   https://mas.owasp.org/MASTG/0x05d-Testing-Data-Storage/

6. Rust WebSocket ecosystem references:
   https://docs.rs/tokio-websockets/

