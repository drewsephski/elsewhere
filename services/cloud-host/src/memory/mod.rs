//! Automatic + searchable Bot memory. Distinct from owner-authored pinned context.

pub mod db;
pub mod extraction;
pub mod retrieval;
pub mod secrets;
pub mod service;
pub mod snapshot;
pub mod types;

pub use db::{
    archive_memory, count_active_memories, get_memory_for_owner, insert_memory, list_memories,
    mark_reinforced, mark_superseded, search_memories, update_memory, ActiveMemoryRef,
    MemoryFilters, MemoryRecord, NewMemory,
};
pub use extraction::{
    enqueue_if_eligible, recover_stale_jobs, tick as tick_extraction, ExtractionError,
    ExtractionModelResponse, ExtractionRequest, EXTRACTION_MAX_CHANGES,
};
pub use retrieval::{
    retrieve_relevant_memories, retrieve_scored, retrieve_scored_in_tx, RetrievedMemory,
    MAX_RUN_MEMORY_BYTES, MAX_RUN_MEMORY_ITEMS,
};
pub use secrets::{looks_like_secret, reject_secret_content};
pub use service::PostgresAgentMemory;
pub use snapshot::{
    persist_run_memories, run_memory_snapshots, RunMemorySnapshot, MEMORY_CONTEXT_PREFACE,
};
pub use types::{
    MemoryKind, MemorySourceKind, MemoryStatus, MAX_ACTIVE_MEMORIES_PER_BOT,
    MAX_MEMORY_CONTENT_BYTES,
};
