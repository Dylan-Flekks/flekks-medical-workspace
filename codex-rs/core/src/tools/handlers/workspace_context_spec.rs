use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use std::collections::BTreeMap;

pub const WORKSPACE_CONTEXT_READ_TOOL_NAME: &str = "workspace_context_read";

pub fn create_workspace_context_read_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "run_id".to_string(),
            JsonSchema::string(Some(
                "Required. Running medical agent run ID bound to a submitted context packet."
                    .to_string(),
            )),
        ),
        (
            "category".to_string(),
            JsonSchema::string_enum(
                vec![
                    serde_json::json!("visit_history"),
                    serde_json::json!("progress_notes"),
                ],
                Some(
                    "Required packet-authorized record category to read.".to_string(),
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
        description: "Read exact immutable chart snapshots for a running medical agent run. The submitted packet must explicitly authorize the requested category; patient and note ownership are derived from the run and cannot be supplied by the model."
            .to_string(),
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
}
