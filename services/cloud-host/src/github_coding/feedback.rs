use serde_json::{json, Value};

const MAX_REVIEW_ITEMS: usize = 50;
const MAX_COMMENT_BODY_CHARS: usize = 4_000;
const MAX_CHECK_ITEMS: usize = 30;

pub fn collect_pull_request_feedback(
    pull: &Value,
    reviews: &[Value],
    review_comments: &[Value],
    issue_comments: &[Value],
    combined_status: &Value,
    check_runs_body: &Value,
) -> Value {
    let head_sha = pull
        .get("head")
        .and_then(|h| h.get("sha"))
        .and_then(|s| s.as_str())
        .unwrap_or("");
    let state = pull.get("state").and_then(|s| s.as_str()).unwrap_or("");
    let title = pull.get("title").and_then(|s| s.as_str()).unwrap_or("");
    let body = truncate_optional(
        pull.get("body").and_then(|s| s.as_str()),
        MAX_COMMENT_BODY_CHARS,
    );

    let reviews_may_be_truncated = reviews.len() >= MAX_REVIEW_ITEMS;
    let review_summaries = reviews
        .iter()
        .take(MAX_REVIEW_ITEMS)
        .filter_map(summarize_review)
        .collect::<Vec<_>>();

    let review_comments_may_be_truncated = review_comments.len() >= MAX_REVIEW_ITEMS;
    let inline_comments = review_comments
        .iter()
        .take(MAX_REVIEW_ITEMS)
        .filter_map(summarize_review_comment)
        .collect::<Vec<_>>();

    let issue_comments_may_be_truncated = issue_comments.len() >= MAX_REVIEW_ITEMS;
    let general_comments = issue_comments
        .iter()
        .take(MAX_REVIEW_ITEMS)
        .filter_map(summarize_issue_comment)
        .collect::<Vec<_>>();

    let checks = summarize_checks(combined_status, check_runs_body);

    json!({
        "contentTrust": "untrusted",
        "truncated": {
            "reviewsMayBeTruncated": reviews_may_be_truncated,
            "reviewCommentsMayBeTruncated": review_comments_may_be_truncated,
            "issueCommentsMayBeTruncated": issue_comments_may_be_truncated,
            "checksMayBeTruncated": checks
                .get("checksMayBeTruncated")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        },
        "pullRequest": {
            "number": pull.get("number"),
            "state": state,
            "title": title,
            "body": body,
            "url": pull.get("html_url"),
            "headRef": pull.get("head").and_then(|h| h.get("ref")),
            "headSha": head_sha,
            "baseRef": pull.get("base").and_then(|b| b.get("ref")),
        },
        "reviews": review_summaries,
        "reviewComments": inline_comments,
        "issueComments": general_comments,
        "checks": checks,
        "phase": "reading_review_feedback"
    })
}

fn summarize_review(review: &Value) -> Option<Value> {
    let state = review.get("state").and_then(|s| s.as_str())?;
    let user = review
        .get("user")
        .and_then(|u| u.get("login"))
        .and_then(|s| s.as_str())
        .unwrap_or("unknown");
    let body = truncate_optional(
        review.get("body").and_then(|s| s.as_str()),
        MAX_COMMENT_BODY_CHARS,
    );
    Some(json!({
        "id": review.get("id"),
        "reviewer": user,
        "state": state,
        "body": body,
        "submittedAt": review.get("submitted_at"),
        "url": review.get("html_url"),
    }))
}

fn summarize_review_comment(comment: &Value) -> Option<Value> {
    let user = comment
        .get("user")
        .and_then(|u| u.get("login"))
        .and_then(|s| s.as_str())
        .unwrap_or("unknown");
    let path = comment.get("path").and_then(|s| s.as_str()).unwrap_or("");
    let body = truncate_optional(
        comment.get("body").and_then(|s| s.as_str()),
        MAX_COMMENT_BODY_CHARS,
    );
    Some(json!({
        "id": comment.get("id"),
        "reviewer": user,
        "path": path,
        "line": comment.get("line").or_else(|| comment.get("original_line")),
        "body": body,
        "url": comment.get("html_url"),
        "inReplyToId": comment.get("in_reply_to_id"),
    }))
}

fn summarize_issue_comment(comment: &Value) -> Option<Value> {
    let user = comment
        .get("user")
        .and_then(|u| u.get("login"))
        .and_then(|s| s.as_str())
        .unwrap_or("unknown");
    let body = truncate_optional(
        comment.get("body").and_then(|s| s.as_str()),
        MAX_COMMENT_BODY_CHARS,
    );
    Some(json!({
        "id": comment.get("id"),
        "author": user,
        "body": body,
        "url": comment.get("html_url"),
    }))
}

fn summarize_checks(combined_status: &Value, check_runs_body: &Value) -> Value {
    let overall_state = combined_status
        .get("state")
        .and_then(|s| s.as_str())
        .unwrap_or("unknown");
    let status_list = combined_status.get("statuses").and_then(|v| v.as_array());
    let statuses_truncated = status_list
        .map(|arr| arr.len() > MAX_CHECK_ITEMS)
        .unwrap_or(false);
    let statuses = status_list
        .map(|arr| {
            arr.iter()
                .take(MAX_CHECK_ITEMS)
                .filter_map(|s| {
                    Some(json!({
                        "context": s.get("context"),
                        "state": s.get("state"),
                        "description": truncate_optional(
                            s.get("description").and_then(|d| d.as_str()),
                            500,
                        ),
                        "targetUrl": s.get("target_url"),
                    }))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let check_run_list = check_runs_body.get("check_runs").and_then(|v| v.as_array());
    let check_runs_truncated = check_run_list
        .map(|arr| arr.len() > MAX_CHECK_ITEMS)
        .unwrap_or(false);
    let check_runs = check_run_list
        .map(|arr| {
            arr.iter()
                .take(MAX_CHECK_ITEMS)
                .filter_map(|run| {
                    Some(json!({
                        "id": run.get("id"),
                        "name": run.get("name"),
                        "status": run.get("status"),
                        "conclusion": run.get("conclusion"),
                        "detailsUrl": run.get("details_url"),
                        "htmlUrl": run.get("html_url"),
                    }))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let failed = check_runs
        .iter()
        .filter(|r| {
            r.get("conclusion")
                .and_then(|c| c.as_str())
                .map(|c| c == "failure" || c == "timed_out" || c == "cancelled")
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    json!({
        "combinedState": overall_state,
        "statuses": statuses,
        "checkRuns": check_runs,
        "failedCheckRuns": failed,
        "checksMayBeTruncated": statuses_truncated || check_runs_truncated,
        "phase": "inspecting_failed_checks"
    })
}

fn truncate_optional(value: Option<&str>, max: usize) -> Option<String> {
    value.map(|s| truncate_str(s, max))
}

fn truncate_str(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &value[..end])
}
