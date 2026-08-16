use crate::event::{ModelMessage, SessionEvent, Surface};

/// Derive model-visible messages from a session log.
///
/// This is the core of the session log invariant: the derived messages
/// MUST match what was actually sent to the LLM.
pub fn derive_messages(log: &[SessionEvent]) -> Vec<ModelMessage> {
    log.iter().filter_map(|e| e.project()).collect()
}

/// Assert that a request's messages match what the session log derives.
///
/// This invariant (model-visible ⟺ logged) is enforced at runtime.
/// Panics in both debug and release builds.
pub fn assert_invariant(request_messages: &[ModelMessage], log: &[SessionEvent]) {
    let derived = derive_messages(log);
    assert_eq!(
        request_messages.len(),
        derived.len(),
        "session log invariant violated: request has {} messages but log derives {}",
        request_messages.len(),
        derived.len()
    );
    for (i, (req, der)) in request_messages.iter().zip(derived.iter()).enumerate() {
        assert_eq!(
            req, der,
            "session log invariant violated at message {i}: request={req:?}, derived={der:?}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_msg(seq: u64, content: &str) -> SessionEvent {
        SessionEvent::UserMessage {
            seq,
            time: 1000,
            content: content.into(),
            turn: Some(1),
            source: None,
        }
    }

    fn assistant_msg(seq: u64, content: &str) -> SessionEvent {
        SessionEvent::AssistantMessage {
            seq,
            time: 1000,
            turn: 1,
            step: 1,
            message: content.into(),
            usage: None,
            source_event_seqs: None,
        }
    }

    fn chunk(seq: u64, content: &str) -> SessionEvent {
        SessionEvent::AssistantChunk {
            seq,
            time: 1000,
            turn: 1,
            step: 1,
            content: content.into(),
        }
    }

    #[test]
    fn derive_empty_log() {
        let messages = derive_messages(&[]);
        assert!(messages.is_empty());
    }

    #[test]
    fn derive_user_and_assistant() {
        let log = vec![user_msg(1, "hello"), assistant_msg(2, "hi there")];
        let messages = derive_messages(&log);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, crate::event::Role::User);
        assert_eq!(messages[1].role, crate::event::Role::Assistant);
    }

    #[test]
    fn chunks_do_not_project() {
        let log = vec![chunk(1, "partial")];
        let messages = derive_messages(&log);
        assert!(messages.is_empty());
    }

    #[test]
    fn lifecycle_events_do_not_project() {
        let log = vec![
            SessionEvent::TurnStart { seq: 1, time: 1000, turn: 1 },
            SessionEvent::StepStart { seq: 2, time: 1001, turn: 1, step: 1 },
            user_msg(3, "test"),
            SessionEvent::StepEnd { seq: 4, time: 1002, turn: 1, step: 1 },
            SessionEvent::TurnEnd { seq: 5, time: 1003, turn: 1, reason: crate::event::TurnEndReason::Completed },
        ];
        let messages = derive_messages(&log);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "test");
    }

    #[test]
    fn invariant_holds_for_matching() {
        let log = vec![user_msg(1, "test")];
        let messages = derive_messages(&log);
        assert_invariant(&messages, &log);
    }

    #[test]
    #[should_panic(expected = "session log invariant violated")]
    fn invariant_rejects_mismatch() {
        let log = vec![user_msg(1, "test")];
        let fake = vec![ModelMessage {
            role: crate::event::Role::User,
            content: "different".into(),
        }];
        assert_invariant(&fake, &log);
    }
}