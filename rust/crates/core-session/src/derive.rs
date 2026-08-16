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

    #[test]
    fn derive_empty_log() {
        let messages = derive_messages(&[]);
        assert!(messages.is_empty());
    }

    #[test]
    fn derive_user_and_assistant() {
        let log = vec![
            SessionEvent::UserMessage {
                content: "hello".into(),
            },
            SessionEvent::AssistantMessage {
                content: "hi there".into(),
            },
        ];
        let messages = derive_messages(&log);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, crate::event::Role::User);
        assert_eq!(messages[1].role, crate::event::Role::Assistant);
    }

    #[test]
    fn chunks_do_not_project() {
        let log = vec![SessionEvent::AssistantChunk {
            content: "partial".into(),
        }];
        let messages = derive_messages(&log);
        assert!(messages.is_empty());
    }

    #[test]
    fn invariant_holds_for_matching() {
        let log = vec![SessionEvent::UserMessage {
            content: "test".into(),
        }];
        let messages = derive_messages(&log);
        assert_invariant(&messages, &log); // should not panic
    }

    #[test]
    #[should_panic(expected = "session log invariant violated")]
    fn invariant_rejects_mismatch() {
        let log = vec![SessionEvent::UserMessage {
            content: "test".into(),
        }];
        let fake = vec![ModelMessage {
            role: crate::event::Role::User,
            content: "different".into(),
        }];
        assert_invariant(&fake, &log);
    }
}
