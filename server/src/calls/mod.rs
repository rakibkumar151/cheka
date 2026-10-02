use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CallState {
    Idle,
    Initiating,
    Ringing,
    Accepted,
    Negotiating,
    Connecting,
    Connected,
    Reconnecting,
    Ending,
    Ended,
    Rejected,
    Missed,
    Failed,
    Cancelled,
}

impl CallState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            CallState::Ended
                | CallState::Rejected
                | CallState::Missed
                | CallState::Failed
                | CallState::Cancelled
        )
    }

    #[allow(clippy::match_like_matches_macro)]
    pub fn can_transition_to(&self, next: &CallState) -> bool {
        if self.is_terminal() {
            return false;
        }

        match (self, next) {
            (CallState::Idle, CallState::Initiating) => true,
            (CallState::Initiating, CallState::Ringing) => true,
            (CallState::Initiating, CallState::Cancelled) => true,

            (CallState::Ringing, CallState::Accepted) => true,
            (CallState::Ringing, CallState::Rejected) => true,
            (CallState::Ringing, CallState::Missed) => true,
            (CallState::Ringing, CallState::Cancelled) => true,

            (CallState::Accepted, CallState::Negotiating) => true,
            (CallState::Accepted, CallState::Ending) => true,
            (CallState::Accepted, CallState::Failed) => true,

            (CallState::Negotiating, CallState::Connecting) => true,
            (CallState::Negotiating, CallState::Ending) => true,
            (CallState::Negotiating, CallState::Failed) => true,

            (CallState::Connecting, CallState::Connected) => true,
            (CallState::Connecting, CallState::Ending) => true,
            (CallState::Connecting, CallState::Failed) => true,

            (CallState::Connected, CallState::Reconnecting) => true,
            (CallState::Connected, CallState::Ending) => true,
            (CallState::Connected, CallState::Failed) => true,

            (CallState::Reconnecting, CallState::Connected) => true,
            (CallState::Reconnecting, CallState::Ending) => true,
            (CallState::Reconnecting, CallState::Failed) => true,

            (CallState::Ending, CallState::Ended) => true,

            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions() {
        assert!(CallState::Idle.can_transition_to(&CallState::Initiating));
        assert!(CallState::Ringing.can_transition_to(&CallState::Accepted));
        assert!(CallState::Connected.can_transition_to(&CallState::Ending));
    }

    #[test]
    fn test_invalid_transitions() {
        assert!(!CallState::Idle.can_transition_to(&CallState::Connected));
        assert!(!CallState::Accepted.can_transition_to(&CallState::Ringing));
    }

    #[test]
    fn test_terminal_states() {
        assert!(!CallState::Ended.can_transition_to(&CallState::Idle));
        assert!(!CallState::Rejected.can_transition_to(&CallState::Accepted));
        assert!(!CallState::Failed.can_transition_to(&CallState::Reconnecting));
    }
}
