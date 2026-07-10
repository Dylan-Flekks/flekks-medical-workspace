use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use std::collections::BTreeMap;

pub const WORKSPACE_CONTEXT_READ_TOOL_NAME: &str = "workspace_context_read";

pub fn create_workspace_context_read_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "client_id".to_string(),
            JsonSchema::string(Some(
                "Required. Saved workspace client ID selected by the user.".to_string(),
            )),
        ),
        (
            "note_id".to_string(),
            JsonSchema::string(Some(
                "Optional saved workspace note ID selected by the user.".to_string(),
            )),
        ),
        (
            "include_documents".to_string(),
            JsonSchema::boolean(Some(
                "Optional. Include linked document metadata; defaults to true.".to_string(),
            )),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: WORKSPACE_CONTEXT_READ_TOOL_NAME.to_string(),
        description: "Read selected workspace context by explicit saved IDs. This tool is read-only, does not list all clients, and returns document metadata plus compact open task metadata."
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            /*required*/ Some(vec!["client_id".to_string()]),
            Some(false.into()),
        ),
        output_schema: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_context_read_requires_client_id() {
        let ToolSpec::Function(tool) = create_workspace_context_read_tool() else {
            panic!("workspace context read should be a function tool");
        };

        assert_eq!(tool.name, WORKSPACE_CONTEXT_READ_TOOL_NAME);
        assert_eq!(
            tool.parameters.required,
            Some(vec!["client_id".to_string()])
        );
        assert!(
            tool.parameters
                .properties
                .as_ref()
                .expect("parameters should have properties")
                .contains_key("note_id")
        );
        assert!(
            tool.parameters
                .properties
                .as_ref()
                .expect("parameters should have properties")
                .contains_key("include_documents")
        );
    }
}
