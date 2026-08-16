use super::trait_def::SandboxBackend;

/// Resolution for sandbox availability.
///
/// The `Unavailable` variant forces callers to handle the case where
/// sandboxing is not available — this is a compile-time guarantee
/// via exhaustive matching.
pub enum SandboxResolution {
    /// Sandbox is available with this backend.
    Confined(Box<dyn SandboxBackend>),
    /// Sandbox is not available. Caller must decide: reject or escalate.
    Unavailable {
        requested: super::trait_def::SandboxPolicy,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_requires_handling() {
        let resolution = SandboxResolution::Unavailable {
            requested: super::super::trait_def::SandboxPolicy::Confined,
        };
        // The compiler forces us to match both variants.
        match resolution {
            SandboxResolution::Confined(_) => {
                unreachable!("test only creates Unavailable")
            }
            SandboxResolution::Unavailable { requested } => {
                assert_eq!(requested, super::super::trait_def::SandboxPolicy::Confined);
            }
        }
    }
}
