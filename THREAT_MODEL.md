# Threat Model

## 1. Threat Actors
- Malicious client (tampered APK)
- Stolen session / Replay attacker
- Compromised device
- API scraper / Malformed WebSocket client
- Database credential thief
- Leaked build artifact

## 2. Protected Assets
- User privacy (call metadata, presence)
- Media contents (audio, video streams)
- Infrastructure credentials (Turso Token, TURN secret, Database URLs)
- Server infrastructure (preventing DoS/OOM)

## 3. Threat Vectors & Mitigations
### A. Hardcoded Secrets in APK
**Threat:** Reverse engineering APK exposes Turn secrets, DB tokens, or API keys.
**Mitigation:** The Android client contains ZERO hardcoded secrets. All secrets are kept on the Rust backend. TURN credentials are generated short-lived per-call.

### B. Man in the Middle (MitM) & Eavesdropping
**Threat:** Intercepting signaling or WebRTC media.
**Mitigation:**
- All API and WebSocket signaling uses TLS 1.3 (HTTPS/WSS).
- WebRTC enforces DTLS/SRTP for media payload encryption.

### C. Malicious Client & SDUI Tampering
**Threat:** A tampered client sends arbitrary commands, or attempts Remote Code Execution (RCE) via UI updates.
**Mitigation:**
- SDUI is strictly declarative (no scripts, Kotlin/Java bytecodes).
- The client maintains a strict whitelist of known actions.
- The server performs deep authorization for every incoming WebSocket message. Client-provided IDs are never trusted over the authenticated session.

### D. DoS and Resource Exhaustion
**Threat:** 100k malicious WebSockets flood the server or database.
**Mitigation:**
- Rate limiting implemented via Redis (per IP, per user session).
- Bounded queues on Tokio channels to apply backpressure.
- Media never proxies through the Rust backend.

### E. Privilege Escalation & State Manipulation
**Threat:** Forcing an unauthorized call state transition (e.g., accepting a call meant for someone else).
**Mitigation:**
- Strict server-side Call State Machine.
- Every state transition requires validation of the caller/callee session ownership and current call state.
