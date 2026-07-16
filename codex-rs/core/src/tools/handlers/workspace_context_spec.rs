use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use std::collections::BTreeMap;

pub const WORKSPACE_CONTEXT_READ_TOOL_NAME: &str = "workspace_context_read";

pub fn create_workspace_context_read_tool() -> ToolSpec {
    create_workspace_context_read_tool_for_categories(
        vec![
            serde_json::json!("visit_history"),
            serde_json::json!("progress_notes"),
        ],
        "Read exact immutable chart snapshots for the current restricted medical run. Patient ownership and the authorized source checkpoint are derived from the immutable run binding and cannot be supplied by the model.",
    )
}

pub fn create_workspace_planning_context_read_tool() -> ToolSpec {
    create_workspace_context_read_tool_for_categories(
        vec![
            serde_json::json!("visit_history"),
            serde_json::json!("progress_notes"),
            serde_json::json!("patient_chart"),
            serde_json::json!("selected_context"),
        ],
        "Read exact patient-scoped snapshots authorized by the immutable planning-run checkpoint. Patient chart data and selected context remain read-only; the model cannot supply or change patient ownership.",
    )
}

fn create_workspace_context_read_tool_for_categories(
    categories: Vec<serde_json::Value>,
    description: &str,
) -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "run_id".to_string(),
            JsonSchema::string(Some(
                "Required. Medical agent or patient-planning run ID bound to this restricted turn."
                    .to_string(),
            )),
        ),
        (
            "category".to_string(),
            JsonSchema::string_enum(
                categories,
                Some(
                    "Required patient-scoped record category authorized by the bound run."
                        .to_string(),
                ),
            ),
        ),
        (
            "limit".to_string(),
            JsonSchema::integer(Some(
                    "Optional maximum records. Defaults to 20, is capped at 100, and cannot exceed the packet scope."
                        .to_string(),
                )),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: WORKSPACE_CONTEXT_READ_TOOL_NAME.to_string(),
        description: description.to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            /*required*/ Some(vec!["run_id".to_string(), "category".to_string()]),
            Some(false.into()),
        ),
        output_schema: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_context_read_requires_run_and_category_only() {
        let ToolSpec::Function(tool) = create_workspace_context_read_tool() else {
            panic!("workspace context read should be a function tool");
        };

        assert_eq!(tool.name, WORKSPACE_CONTEXT_READ_TOOL_NAME);
        assert_eq!(
            tool.parameters.required,
            Some(vec!["run_id".to_string(), "category".to_string()])
        );
        let properties = tool
            .parameters
            .properties
            .as_ref()
            .expect("parameters should have properties");
        assert_eq!(
            properties.keys().cloned().collect::<Vec<_>>(),
            vec![
                "category".to_string(),
                "limit".to_string(),
                "run_id".to_string()
            ]
        );
        assert_eq!(
            properties["category"].enum_values,
            Some(vec![
                serde_json::json!("visit_history"),
                serde_json::json!("progress_notes")
            ])
        );
    }

    #[test]
    fn workspace_planning_context_read_adds_only_bounded_patient_categories() {
        let ToolSpec::Function(tool) = create_workspace_planning_context_read_tool() else {
            panic!("workspace planning context read should be a function tool");
        };
        let properties = tool
            .parameters
            .properties
            .as_ref()
            .expect("parameters should have properties");
        assert_eq!(
            properties["category"].enum_values,
            Some(vec![
                serde_json::json!("visit_history"),
                serde_json::json!("progress_notes"),
                serde_json::json!("patient_chart"),
                serde_json::json!("selected_context"),
            ])
        );
    }
}
