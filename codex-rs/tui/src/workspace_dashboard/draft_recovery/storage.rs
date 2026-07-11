use super::*;
use std::collections::BTreeSet;

pub(super) enum RecoveryListScope {
    AllActive,
    ClientIncludingClosed(String),
}

pub(super) async fn list_recovery_sessions(
    app_server: &mut AppServerSession,
    scope: RecoveryListScope,
) -> Result<Vec<WorkspaceDraftSession>> {
    let mut cursor = None;
    let mut seen_cursors = BTreeSet::new();
    let mut seen_session_ids = BTreeSet::new();
    let mut sessions = Vec::new();
    loop {
        let (client_id, all_clients, include_closed) = match &scope {
            RecoveryListScope::AllActive => (None, true, false),
            RecoveryListScope::ClientIncludingClosed(client_id) => {
                (Some(client_id.clone()), false, true)
            }
        };
        let response = app_server
            .workspace_draft_session_list(WorkspaceDraftSessionListParams {
                client_id,
                all_clients,
                include_closed,
                cursor: cursor.clone(),
                limit: Some(100),
            })
            .await?;
        for session in response.data {
            validate_recovery_session_envelope(
                &session,
                matches!(&scope, RecoveryListScope::AllActive),
            )?;
            if seen_session_ids.insert(session.id.clone()) {
                sessions.push(session);
            }
        }
        let Some(next_cursor) = response.next_cursor else {
            return Ok(sessions);
        };
        if !seen_cursors.insert(next_cursor.clone()) {
            color_eyre::eyre::bail!("workspace draft recovery cursor loop detected");
        }
        cursor = Some(next_cursor);
    }
}
