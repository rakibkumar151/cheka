# Test Plan

## 1. Unit Tests (Rust Backend)
- **Auth & Session:** Validate token generation, expiration, and revocation.
- **State Machine:** Verify correct and reject invalid call state transitions (e.g., RINGING -> ACCEPTED).
- **SDUI Validation:** Ensure the SDUI document parser rejects unknown components and actions.
- **Rate Limiting:** Verify Redis-backed limits correctly shed load.

## 2. Integration Tests (Backend)
- Login -> Bootstrap -> WS connect -> Authenticated session flow.
- Call start -> Notification -> Accept -> Negotiation sequence.
- End-to-end rejection, cancellation, and missing peer behavior.
- Multi-instance coordination via Redis (Pub/Sub).

## 3. Android Thin-Runtime Tests
- **Permissions:** Verify graceful handling of denied camera/microphone permissions.
- **SDUI Rendering:** Ensure JSON schemas correctly inflate native Android UI components.
- **SDUI Fallback:** Verify rollback to last-known-good schema if parsing fails.
- **Lifecycle:** Handle foreground/background transitions and network loss.

## 4. WebRTC / Media Tests
- Test direct Audio-to-Audio and Video-to-Video connections.
- Simulate ICE candidate exchange and TURN relay fallback.
- Test in-call controls: Camera switch, mute/unmute, and track disablement.

## 5. Security & Threat Tests
- Attempt unauthorized call termination or acceptance (IDOR).
- Replay expired JWTs or WebSocket frames.
- Fuzz WebSocket payloads and oversized frames.
- Secret scanning on Android APK and source tree.

## 6. Load & Performance Tests
- **Target:** 100,000 idle authenticated WebSockets.
- **Burst Test:** High volume of simultaneous call setups.
- **Metrics Tracking:** CPU, RAM, p50/p95/p99 latency, and error rates.
