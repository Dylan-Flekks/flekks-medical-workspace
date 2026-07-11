use super::WorkspaceContextPacketCreateParams;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn context_packet_create_draft_source_round_trips_and_remains_optional() {
    let bound_wire = json!({
        "clientId": "client-1",
        "encounterId": null,
        "noteId": "note-1",
        "sourceDraftSessionId": "session-1",
        "sourceDraftCheckpointId": "checkpoint-1",
        "sourceDraftCheckpointRevision": 3,
        "sourceDraftCheckpointSha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "humanRequest": "Review this draft.",
        "selectedArtifactIdsJson": "[]",
        "selectedDerivativeIdsJson": "[]",
        "selectedClipIdsJson": "[]",
        "artifactSummary": "",
        "derivativeSummary": "",
        "clipSummary": "",
        "chartContextSummary": "",
        "contextEnvelopeJson": "{}",
        "clinicianActor": "Clinician Example",
        "baseNoteRevision": 2,
        "authorizedScopeJson": null,
        "expectedOutputKind": "note_proposal"
    });
    let bound: WorkspaceContextPacketCreateParams =
        serde_json::from_value(bound_wire.clone()).expect("bound params should deserialize");
    assert_eq!(
        serde_json::to_value(bound).expect("bound params should serialize"),
        bound_wire
    );

    let legacy: WorkspaceContextPacketCreateParams = serde_json::from_value(json!({
        "clientId": "client-1",
        "humanRequest": "Review this draft.",
        "selectedArtifactIdsJson": "[]",
        "selectedDerivativeIdsJson": "[]",
        "selectedClipIdsJson": "[]",
        "artifactSummary": "",
        "derivativeSummary": "",
        "clipSummary": "",
        "chartContextSummary": "",
        "contextEnvelopeJson": "{}"
    }))
    .expect("legacy params should deserialize without a draft source");
    assert_eq!(
        (
            legacy.source_draft_session_id,
            legacy.source_draft_checkpoint_id,
            legacy.source_draft_checkpoint_revision,
            legacy.source_draft_checkpoint_sha256,
        ),
        (None, None, None, None)
    );
}
