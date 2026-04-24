use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::Response,
};
use std::sync::Arc;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use super::routes::AppState;

pub async fn events(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Response<Body>, StatusCode> {
    let rx = state.solver.subscribe(&id).ok_or(StatusCode::NOT_FOUND)?;
    let bootstrap_json = state
        .solver
        .bootstrap_event(&id)
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let bootstrap_event_sequence = event_sequence_from_json(&bootstrap_json);
    let bootstrap = tokio_stream::iter(std::iter::once(Ok::<_, std::convert::Infallible>(
        format!("data: {}\n\n", bootstrap_json).into_bytes(),
    )));

    let live = BroadcastStream::new(rx).filter_map(move |msg| match msg {
        Ok(json) => {
            if event_is_not_newer(&json, bootstrap_event_sequence) {
                return None;
            }
            Some(Ok::<_, std::convert::Infallible>(
                format!("data: {}\n\n", json).into_bytes(),
            ))
        }
        Err(_) => None, // Lagged - skip missed messages
    });

    let stream = bootstrap.chain(live);

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .header("X-Accel-Buffering", "no")
        .body(Body::from_stream(stream))
        .unwrap())
}

fn event_sequence_from_json(json: &str) -> Option<u64> {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|value| {
            value
                .get("eventSequence")
                .and_then(serde_json::Value::as_u64)
        })
}

fn event_is_not_newer(json: &str, bootstrap_event_sequence: Option<u64>) -> bool {
    let Some(bootstrap_event_sequence) = bootstrap_event_sequence else {
        return false;
    };
    event_sequence_from_json(json)
        .is_some_and(|event_sequence| event_sequence <= bootstrap_event_sequence)
}
