/// Headless runner — CLI-based agent runner without HTTP surface.
///
/// Phase 4 will implement the full CLI runner.
pub struct HeadlessRunner {
    // Placeholder — will hold AgentRunner + config.
}

impl HeadlessRunner {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for HeadlessRunner {
    fn default() -> Self {
        Self::new()
    }
}
