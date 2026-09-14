//! Structured phase timings for Codex subscription runs (logged, not hot-path).

use std::time::Instant;

#[derive(Debug)]
pub struct RunPhaseRecorder {
    run_started: Instant,
    codex_launch: Option<Instant>,
    thread_open: Option<Instant>,
    turn_start: Option<Instant>,
    first_assistant_delta: Option<Instant>,
    first_tool: Option<Instant>,
    completed: Option<Instant>,
}

impl RunPhaseRecorder {
    pub fn new() -> Self {
        Self {
            run_started: Instant::now(),
            codex_launch: None,
            thread_open: None,
            turn_start: None,
            first_assistant_delta: None,
            first_tool: None,
            completed: None,
        }
    }

    pub fn mark_codex_launch(&mut self) {
        self.codex_launch.get_or_insert(Instant::now());
    }

    pub fn mark_thread_open(&mut self) {
        self.thread_open.get_or_insert(Instant::now());
    }

    pub fn mark_turn_start(&mut self) {
        self.turn_start.get_or_insert(Instant::now());
    }

    pub fn mark_first_assistant_delta(&mut self) {
        self.first_assistant_delta.get_or_insert(Instant::now());
    }

    pub fn mark_first_tool(&mut self) {
        self.first_tool.get_or_insert(Instant::now());
    }

    pub fn mark_completed(&mut self) {
        self.completed.get_or_insert(Instant::now());
    }

    pub fn log_summary(&self, request_id: &str) {
        let ms = |start: Instant, end: Option<Instant>| -> Option<u128> {
            end.map(|t| t.duration_since(start).as_millis())
        };
        tracing::info!(
            target: "elsewhere_run_phases",
            request_id = %request_id,
            queue_to_codex_launch_ms = ?ms(self.run_started, self.codex_launch),
            codex_launch_to_thread_open_ms = ?self
                .codex_launch
                .zip(self.thread_open)
                .map(|(a, b)| b.duration_since(a).as_millis()),
            thread_open_to_turn_start_ms = ?self
                .thread_open
                .zip(self.turn_start)
                .map(|(a, b)| b.duration_since(a).as_millis()),
            turn_start_to_first_assistant_delta_ms = ?self
                .turn_start
                .zip(self.first_assistant_delta)
                .map(|(a, b)| b.duration_since(a).as_millis()),
            turn_start_to_first_tool_ms = ?self
                .turn_start
                .zip(self.first_tool)
                .map(|(a, b)| b.duration_since(a).as_millis()),
            turn_start_to_completed_ms = ?self
                .turn_start
                .zip(self.completed)
                .map(|(a, b)| b.duration_since(a).as_millis()),
            run_start_to_first_assistant_delta_ms = ?ms(self.run_started, self.first_assistant_delta),
        );
    }
}
