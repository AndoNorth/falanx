use async_trait::async_trait;
use cersei_provider::{CompletionRequest, CompletionStream, Provider, ProviderCapabilities};
use cersei_types::{StopReason, StreamEvent};
use tokio::sync::mpsc;

pub struct MockProvider;

impl MockProvider {
    fn mock_score_json() -> &'static str {
        r#"{"readability":3,"maintainability":3,"performance":3,"security":3,"architecture":3}"#
    }

    fn mock_issues_json() -> &'static str {
        r#"[{"location":"mock:0","problem":"no issues found","fix":"none"}]"#
    }

    fn mock_patches_json() -> &'static str {
        r#"[]"#
    }

    fn response_for(request: &CompletionRequest) -> &'static str {
        let system = request.system.as_deref().unwrap_or("");
        if system.contains("SCORE") {
            Self::mock_score_json()
        } else if system.contains("CRITIQUE") {
            Self::mock_issues_json()
        } else {
            Self::mock_patches_json()
        }
    }

    fn static_stream(text: &'static str) -> CompletionStream {
        let (tx, rx) = mpsc::channel(16);
        tokio::spawn(async move {
            let _ = tx
                .send(StreamEvent::MessageStart {
                    id: "mock-id".to_string(),
                    model: "mock".to_string(),
                })
                .await;
            let _ = tx
                .send(StreamEvent::ContentBlockStart {
                    index: 0,
                    block_type: "text".to_string(),
                    id: None,
                    name: None,
                })
                .await;
            let _ = tx
                .send(StreamEvent::TextDelta {
                    index: 0,
                    text: text.to_string(),
                })
                .await;
            let _ = tx.send(StreamEvent::ContentBlockStop { index: 0 }).await;
            let _ = tx
                .send(StreamEvent::MessageDelta {
                    stop_reason: Some(StopReason::EndTurn),
                    usage: None,
                })
                .await;
            let _ = tx.send(StreamEvent::MessageStop).await;
        });
        CompletionStream::new(rx)
    }
}

#[async_trait]
impl Provider for MockProvider {
    fn name(&self) -> &str {
        "mock"
    }

    fn context_window(&self, _model: &str) -> u64 {
        200_000
    }

    fn capabilities(&self, _model: &str) -> ProviderCapabilities {
        ProviderCapabilities::default()
    }

    async fn complete(
        &self,
        request: CompletionRequest,
    ) -> cersei_types::Result<CompletionStream> {
        Ok(Self::static_stream(Self::response_for(&request)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_provider_implements_provider_trait() {
        fn assert_provider<T: Provider>() {}
        assert_provider::<MockProvider>();
    }

    #[test]
    fn mock_provider_name() {
        assert_eq!(MockProvider.name(), "mock");
    }

    #[test]
    fn response_routing_score() {
        let mut req = CompletionRequest::new("mock");
        req.system = Some("SCORE this diff".to_string());
        let resp = MockProvider::response_for(&req);
        assert!(resp.contains("readability"));
    }

    #[test]
    fn response_routing_critique() {
        let mut req = CompletionRequest::new("mock");
        req.system = Some("CRITIQUE this diff".to_string());
        let resp = MockProvider::response_for(&req);
        assert!(resp.contains("location"));
    }

    #[test]
    fn response_routing_rewrite_default() {
        let req = CompletionRequest::new("mock");
        let resp = MockProvider::response_for(&req);
        assert_eq!(resp, "[]");
    }
}
