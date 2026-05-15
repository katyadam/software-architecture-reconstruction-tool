use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    },
};

pub use crate::resolver::query::{QueryKind, SageQuery};
pub use crate::resolver::response::{SageError, SageResponse};

use crate::resolver::{
    prompt::{
        build_facts_message, build_question_message, build_system_message, build_variables_message,
    },
    response::{LlmJson, validate},
};

/// HTTP client that sends resolution queries to an Ollama-backed LLM.
#[derive(Debug)]
pub struct SageClient {
    client: Client<OpenAIConfig>,
    model: String,
    confidence_threshold: f32,
}

impl SageClient {
    /// Create a client pointing at `base_url` (e.g. `"http://localhost:11434/v1"`).
    pub fn new(base_url: &str, model: &str, confidence_threshold: f32) -> Self {
        let config = OpenAIConfig::new()
            .with_api_base(base_url)
            .with_api_key("ollama");
        Self {
            client: Client::with_config(config),
            model: model.to_string(),
            confidence_threshold,
        }
    }

    /// Send a resolution query and return a validated response.
    pub async fn query(&self, query: SageQuery) -> Result<SageResponse, SageError> {
        let system_msg = build_system_message();
        let facts_msg = build_facts_message(&query.bundle);
        let variables_msg = build_variables_message(&query.variables_map);
        let question_msg = build_question_message(&query.kind);

        let messages: Vec<ChatCompletionRequestMessage> = vec![
            ChatCompletionRequestSystemMessageArgs::default()
                .content(system_msg)
                .build()
                .map_err(SageError::Network)?
                .into(),
            ChatCompletionRequestUserMessageArgs::default()
                .content(facts_msg)
                .build()
                .map_err(SageError::Network)?
                .into(),
            ChatCompletionRequestUserMessageArgs::default()
                .content(variables_msg)
                .build()
                .map_err(SageError::Network)?
                .into(),
            ChatCompletionRequestUserMessageArgs::default()
                .content(question_msg)
                .build()
                .map_err(SageError::Network)?
                .into(),
        ];

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages(messages)
            .build()
            .map_err(SageError::Network)?;

        let response = self.client.chat().create(request).await?;

        let text = response
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .unwrap_or_default();

        let parsed: LlmJson = serde_json::from_str(text.trim())?;
        validate(parsed, self.confidence_threshold)
    }
}
