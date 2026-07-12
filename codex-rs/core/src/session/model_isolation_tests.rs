use codex_protocol::models::ContentItem;
use codex_protocol::models::InternalChatMessageMetadataPassthrough;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::SessionSource;
use codex_protocol::user_input::UserInput;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::*;
use crate::config::Config;
use crate::config::ConfigBuilder;

fn creation<'a>(
    config: &'a Config,
    history: &'a InitialHistory,
    session_source: &'a SessionSource,
    extension_init: &'a ExtensionDataInit,
) -> IsolatedThreadCreation<'a> {
    IsolatedThreadCreation {
        config,
        initial_history: history,
        session_source,
        forked_from_thread_id_present: false,
        parent_thread_id_present: false,
        thread_source: None,
        dynamic_tools: &[],
        inherited_environments: None,
        environment_selections: &[],
        thread_extension_init: extension_init,
    }
}

fn strict_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "hint": {"type": "string"},
            "steps": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {"label": {"type": "string"}},
                    "required": ["label"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["hint", "steps"],
        "additionalProperties": false
    })
}

fn assistant(text: impl Into<String>) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText { text: text.into() }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }
}

fn assistant_parts(content: Vec<ContentItem>) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "assistant".to_string(),
        content,
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }
}

#[tokio::test]
async fn isolated_creation_requires_fresh_ephemeral_empty_thread_state() {
    let home = tempfile::tempdir().expect("create temp codex home");
    let mut config = ConfigBuilder::without_managed_config_for_tests()
        .codex_home(home.path().to_path_buf())
        .build()
        .await
        .expect("build test config");
    config.ephemeral = true;
    config.workspace_roots.clear();
    let history = InitialHistory::New;
    let session_source = SessionSource::Mcp;
    let extension_init = ExtensionDataInit::default();
    assert!(
        validate_thread_creation(
            ModelToolMode::Isolated,
            creation(&config, &history, &session_source, &extension_init),
        )
        .is_ok()
    );

    config.ephemeral = false;
    assert!(
        validate_thread_creation(
            ModelToolMode::Isolated,
            creation(&config, &history, &session_source, &extension_init),
        )
        .is_err()
    );
    config.ephemeral = true;
    assert!(
        validate_thread_creation(
            ModelToolMode::Isolated,
            IsolatedThreadCreation {
                initial_history: &InitialHistory::Cleared,
                ..creation(&config, &history, &session_source, &extension_init)
            },
        )
        .is_err()
    );
    assert!(
        validate_thread_creation(
            ModelToolMode::Isolated,
            IsolatedThreadCreation {
                parent_thread_id_present: true,
                ..creation(&config, &history, &session_source, &extension_init)
            },
        )
        .is_err()
    );
}

#[test]
fn isolated_mode_is_creation_only_and_disabled_stays_mutable() {
    assert!(validate_mode_transition(ModelToolMode::Default, ModelToolMode::Disabled).is_ok());
    assert!(validate_mode_transition(ModelToolMode::Disabled, ModelToolMode::Default).is_ok());
    assert!(validate_mode_transition(ModelToolMode::Isolated, ModelToolMode::Isolated).is_ok());
    assert!(validate_mode_transition(ModelToolMode::Default, ModelToolMode::Isolated).is_err());
    assert!(validate_mode_transition(ModelToolMode::Isolated, ModelToolMode::Disabled).is_err());
    assert!(ModelToolMode::Disabled.tools_disabled());
    assert!(ModelToolMode::Isolated.tools_disabled());
    assert!(!ModelToolMode::Disabled.is_isolated());
}

#[test]
fn isolated_input_is_one_nonempty_plain_text_item_bounded_by_utf8_bytes() {
    let exact = "é".repeat(MAX_ISOLATED_INPUT_BYTES / 2);
    assert_eq!(exact.len(), MAX_ISOLATED_INPUT_BYTES);
    assert!(
        validate_single_text_input(&[UserInput::Text {
            text: exact.clone(),
            text_elements: Vec::new(),
        }])
        .is_ok()
    );
    assert!(
        validate_single_text_input(&[UserInput::Text {
            text: format!("{exact}x"),
            text_elements: Vec::new(),
        }])
        .is_err()
    );
    assert!(
        validate_single_text_input(&[UserInput::Text {
            text: "   ".to_string(),
            text_elements: Vec::new(),
        }])
        .is_err()
    );
    assert!(validate_single_text_input(&[]).is_err());
}

#[test]
fn isolated_schema_rejects_permissive_or_unsupported_shapes() {
    let schema = strict_schema();
    assert!(schema::validate_strict_object_schema(&schema).is_ok());
    assert!(
        schema::validate_value(
            &schema,
            &json!({"hint": "Save first", "steps": [{"label": "Submit"}]})
        )
        .is_ok()
    );
    assert!(
        schema::validate_value(
            &schema,
            &json!({"hint": "Save first", "steps": [], "extra": true})
        )
        .is_err()
    );

    for invalid in [
        json!({"type": "object", "properties": {}, "required": []}),
        json!({
            "type": "object",
            "properties": {"hint": {"$ref": "#/$defs/hint"}},
            "required": ["hint"],
            "additionalProperties": false
        }),
        json!({
            "type": "object",
            "properties": {"hint": {"anyOf": [{"type": "string"}]}},
            "required": ["hint"],
            "additionalProperties": false
        }),
    ] {
        assert!(schema::validate_strict_object_schema(&invalid).is_err());
    }
}

#[test]
fn isolated_output_is_one_bounded_schema_valid_assistant_message() {
    let schema = strict_schema();
    let mut state = IsolatedOutputState::default();
    let reasoning = ResponseItem::Reasoning {
        id: None,
        summary: Vec::new(),
        content: None,
        encrypted_content: None,
        internal_chat_message_metadata_passthrough: None,
    };
    assert!(
        validate_model_output(
            ModelToolMode::Isolated,
            &reasoning,
            ModelOutputStage::Completed,
            Some(&schema),
            &mut state,
        )
        .is_ok()
    );
    assert!(
        validate_model_output(
            ModelToolMode::Isolated,
            &assistant(r#"{"hint":"Save first","steps":[]}"#),
            ModelOutputStage::Completed,
            Some(&schema),
            &mut state,
        )
        .is_ok()
    );
    assert!(
        validate_model_output(
            ModelToolMode::Isolated,
            &assistant(r#"{"hint":"Again","steps":[]}"#),
            ModelOutputStage::Completed,
            Some(&schema),
            &mut state,
        )
        .is_err()
    );
    assert!(
        take_validated_assistant_output(ModelToolMode::Isolated, &mut state, Some(false)).is_err()
    );
    assert!(
        take_validated_assistant_output(ModelToolMode::Isolated, &mut state, Some(true))
            .expect("validated terminal output")
            .is_some()
    );

    for invalid in [
        assistant("not json"),
        assistant(r#"{"hint":"missing steps"}"#),
        assistant_parts(vec![
            ContentItem::OutputText {
                text: r#"{"hint":"Save first","#.to_string(),
            },
            ContentItem::OutputText {
                text: r#""steps":[]}"#.to_string(),
            },
        ]),
        ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: r#"{"hint":"Commentary","steps":[]}"#.to_string(),
            }],
            phase: Some(MessagePhase::Commentary),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: r#"{"hint":"Metadata","steps":[]}"#.to_string(),
            }],
            phase: Some(MessagePhase::FinalAnswer),
            internal_chat_message_metadata_passthrough: Some(
                InternalChatMessageMetadataPassthrough {
                    turn_id: Some("unexpected".to_string()),
                },
            ),
        },
        ResponseItem::Other,
    ] {
        let mut state = IsolatedOutputState::default();
        assert!(
            validate_model_output(
                ModelToolMode::Isolated,
                &invalid,
                ModelOutputStage::Completed,
                Some(&schema),
                &mut state,
            )
            .is_err()
        );
    }

    let prefix = "{\"hint\":\"";
    let suffix = "\",\"steps\":[]}";
    let exact = format!(
        "{prefix}{}{suffix}",
        "x".repeat(MAX_ISOLATED_OUTPUT_BYTES - prefix.len() - suffix.len())
    );
    assert_eq!(exact.len(), MAX_ISOLATED_OUTPUT_BYTES);
    let mut exact_state = IsolatedOutputState::default();
    assert!(
        validate_model_output(
            ModelToolMode::Isolated,
            &assistant(exact.clone()),
            ModelOutputStage::Completed,
            Some(&schema),
            &mut exact_state,
        )
        .is_ok()
    );
    let mut oversized_state = IsolatedOutputState::default();
    assert!(
        validate_model_output(
            ModelToolMode::Isolated,
            &assistant(format!("{exact}x")),
            ModelOutputStage::Completed,
            Some(&schema),
            &mut oversized_state,
        )
        .is_err()
    );
}
