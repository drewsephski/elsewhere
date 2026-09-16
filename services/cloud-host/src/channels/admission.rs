//! Admit normalized channel messages through the canonical work path.

use crate::app_state::AppState;
use crate::channels::db::{
    events_in_window, find_connected_slack_workspace, get_or_create_thread, mark_event_status,
};
use crate::channels::slack::events::should_ignore;
use crate::channels::types::NormalizedInbound;
use crate::channels::MAX_EVENTS_PER_CONNECTION_PER_MINUTE;
use crate::channels::MAX_INBOUND_TEXT_BYTES;
use crate::error::ApiError;
use crate::skills::SkillAdmissionInput;
use crate::work_admission::{admit, WorkAdmissionIntent};

pub async fn admit_normalized(
    state: &AppState,
    event_row_id: &str,
    inbound: &NormalizedInbound,
) -> Result<(), ApiError> {
    let Some(connection) =
        find_connected_slack_workspace(&state.pool, &inbound.workspace_id).await?
    else {
        mark_event_status(
            &state.pool,
            event_row_id,
            "ignored",
            Some("no_connection"),
            None,
        )
        .await?;
        return Ok(());
    };

    if let Some(reason) = should_ignore(inbound, connection.bot_user_id.as_deref()) {
        mark_event_status(&state.pool, event_row_id, "ignored", Some(reason), None).await?;
        return Ok(());
    }

    if connection.installer_external_user_id.as_deref() != Some(inbound.sender_user_id.as_str()) {
        mark_event_status(
            &state.pool,
            event_row_id,
            "ignored",
            Some("non_installer"),
            None,
        )
        .await?;
        return Ok(());
    }

    let text = inbound.text.trim();
    if text.is_empty() {
        mark_event_status(&state.pool, event_row_id, "ignored", Some("empty"), None).await?;
        return Ok(());
    }
    if text.len() > MAX_INBOUND_TEXT_BYTES {
        mark_event_status(
            &state.pool,
            event_row_id,
            "rejected",
            Some("oversized"),
            None,
        )
        .await?;
        return Ok(());
    }

    let recent = events_in_window(&state.pool, &connection.id, 60).await?;
    if recent > MAX_EVENTS_PER_CONNECTION_PER_MINUTE {
        mark_event_status(
            &state.pool,
            event_row_id,
            "rejected",
            Some("rate_limited"),
            None,
        )
        .await?;
        return Ok(());
    }

    let Some(bot_id) = connection.default_bot_id.as_deref() else {
        mark_event_status(&state.pool, event_row_id, "ignored", Some("no_bot"), None).await?;
        return Ok(());
    };

    let request_id = format!(
        "{}:{}:{}",
        inbound.provider.as_str(),
        inbound.workspace_id,
        inbound.event_id
    );

    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let thread = get_or_create_thread(
        &mut tx,
        &connection,
        bot_id,
        &inbound.thread.channel_id,
        &inbound.thread.thread_id,
    )
    .await?;
    let skills = SkillAdmissionInput::default();
    let records = match admit(
        &mut tx,
        &connection.owner_id,
        &request_id,
        WorkAdmissionIntent::ChannelMessage {
            bot_id,
            conversation_id: &thread.conversation_id,
            message: text,
            skills: &skills,
            origin_provider: inbound.provider.as_str(),
        },
    )
    .await
    {
        Ok(records) => records,
        Err(err) => {
            tx.rollback()
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            let reason = match err {
                ApiError::TooManyRequests => "queue_limit",
                ApiError::Validation(_) => "validation",
                ApiError::Conflict(_) => "conflict",
                _ => "admission_failed",
            };
            mark_event_status(&state.pool, event_row_id, "rejected", Some(reason), None).await?;
            return Ok(());
        }
    };
    tx.commit()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    mark_event_status(
        &state.pool,
        event_row_id,
        "admitted",
        None,
        Some(&records.run_id),
    )
    .await?;
    Ok(())
}

pub async fn process_pending_event(
    state: &AppState,
    event_row_id: &str,
    inbound: NormalizedInbound,
) {
    if let Err(err) = admit_normalized(state, event_row_id, &inbound).await {
        tracing::warn!(
            error = %crate::redact::redact_secrets(&err.to_string()),
            "channel event admission failed"
        );
    }
}

/// Restart-safe: admit any events left in `received` after a crash between
/// webhook acknowledgement and work admission. Slack retries are also
/// deduplicated by `event_id`.
pub async fn tick_pending(state: &AppState) -> Result<usize, ApiError> {
    let rows = crate::channels::db::list_received_events(&state.pool, 20).await?;
    let mut processed = 0;
    for (event_row_id, payload) in rows {
        let Some(inbound) = NormalizedInbound::from_payload(&payload) else {
            mark_event_status(
                &state.pool,
                &event_row_id,
                "ignored",
                Some("invalid_payload"),
                None,
            )
            .await?;
            continue;
        };
        admit_normalized(state, &event_row_id, &inbound).await?;
        processed += 1;
    }
    Ok(processed)
}
