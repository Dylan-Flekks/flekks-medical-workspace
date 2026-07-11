use super::*;

pub(super) fn validate_selected_context(
    staged: &WorkspaceDashboard,
    session: &WorkspaceDraftSession,
    snapshot: &RecoveredSnapshot,
) -> Result<()> {
    for artifact_id in &snapshot.selected_artifact_ids {
        let document = staged
            .documents
            .iter()
            .find(|document| document.id == *artifact_id)
            .ok_or_else(|| unverified_context_error("file"))?;
        if document.client_id != session.client_id || artifact_scope_label(document) != "patient" {
            return Err(unverified_context_error("file"));
        }
    }
    for derivative_id in &snapshot.selected_derivative_ids {
        let derivative = staged
            .derivatives
            .iter()
            .find(|derivative| derivative.id == *derivative_id)
            .ok_or_else(|| unverified_context_error("reviewed text"))?;
        if derivative.client_id != session.client_id
            || !staged.documents.iter().any(|document| {
                document.id == derivative.document_id
                    && document.client_id == session.client_id
                    && artifact_scope_label(document) == "patient"
            })
        {
            return Err(unverified_context_error("reviewed text"));
        }
    }
    for clip_id in &snapshot.selected_clip_ids {
        let clip = staged
            .clips
            .iter()
            .find(|clip| clip.id == *clip_id)
            .ok_or_else(|| unverified_context_error("context clip"))?;
        let derivative = staged
            .derivatives
            .iter()
            .find(|derivative| derivative.id == clip.derivative_id);
        if clip.client_id != session.client_id
            || derivative.is_none_or(|derivative| {
                derivative.client_id != session.client_id
                    || derivative.document_id != clip.document_id
            })
            || !staged.documents.iter().any(|document| {
                document.id == clip.document_id
                    && document.client_id == session.client_id
                    && artifact_scope_label(document) == "patient"
            })
        {
            return Err(unverified_context_error("context clip"));
        }
    }
    Ok(())
}

fn unverified_context_error(label: &str) -> color_eyre::Report {
    color_eyre::eyre::eyre!(
        "selected {label} context could not be verified; draft retained and discard remains available"
    )
}
