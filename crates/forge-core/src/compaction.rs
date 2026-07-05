//! Compaction de contexto em fronteiras seguras (Fase 2).
//!
//! Conceitos da spec `opencode/CONTEXT.md`: quando o histórico se aproxima
//! da janela de contexto, a conversa inteira é resumida e uma nova época
//! começa com o resumo como baseline. A fronteira é segura quando o último
//! turno do assistente terminou sem tool_use pendente — nunca se corta um
//! par tool_use/tool_result ao meio.
//!
//! A estimativa de tokens é a heurística `chars/4` do fork do opencode
//! (o tokenizer BPE real foi marcado won't-do: ~60× mais lento no hot
//! path para pouco ganho de precisão).

use forge_llm::chat::{ChatMessage, ContentBlock, GenerateRequest, Role};
use forge_llm::gateway::{GatewayError, Generator};
use forge_llm::ModelTier;

/// Estima tokens do histórico: total de caracteres / 4.
pub fn estimate_tokens(messages: &[ChatMessage]) -> usize {
    let chars: usize = messages
        .iter()
        .flat_map(|m| m.content.iter())
        .map(|block| match block {
            ContentBlock::Text { text } => text.len(),
            ContentBlock::ToolUse { name, input, .. } => name.len() + input.to_string().len(),
            ContentBlock::ToolResult { content, .. } => content.len(),
        })
        .sum();
    chars / 4
}

/// Política de compaction: janela de contexto × threshold do tier
/// (small compacta a ~75%, demais a ~90% — tier-gating do fork).
#[derive(Debug, Clone, Copy)]
pub struct CompactionPolicy {
    pub context_window_tokens: usize,
    pub threshold: f64,
}

impl CompactionPolicy {
    pub fn for_tier(tier: ModelTier, context_window_tokens: usize) -> Self {
        Self {
            context_window_tokens,
            threshold: tier.compaction_threshold(),
        }
    }

    /// O histórico ultrapassou o limite da política?
    pub fn needs_compaction(&self, messages: &[ChatMessage]) -> bool {
        let limit = (self.context_window_tokens as f64 * self.threshold) as usize;
        estimate_tokens(messages) >= limit
    }

    /// Fronteira segura: o último turno é do assistente e não deixou
    /// tool_use pendente (turno de provider completo).
    pub fn is_safe_boundary(messages: &[ChatMessage]) -> bool {
        match messages.last() {
            Some(last) if matches!(last.role, Role::Assistant) => !last
                .content
                .iter()
                .any(|b| matches!(b, ContentBlock::ToolUse { .. })),
            _ => false,
        }
    }

    /// Pede ao modelo (sem ferramentas) um resumo da conversa que preserve
    /// decisões, estado dos arquivos e pendências — a baseline da nova época.
    pub async fn summarize<G: Generator>(
        &self,
        generator: &G,
        model: &str,
        messages: &[ChatMessage],
    ) -> Result<String, GatewayError> {
        let mut prompt_messages = messages.to_vec();
        prompt_messages.push(ChatMessage::user_text(
            "Resuma esta conversa para continuar o trabalho em uma nova sessão: \
objetivo, decisões tomadas, arquivos tocados e estado atual, pendências. \
Seja denso e factual; não invente nada.",
        ));
        let turn = generator
            .generate(
                GenerateRequest {
                    model: model.to_string(),
                    system: "Você resume conversas de trabalho de um coding agent.".into(),
                    messages: prompt_messages,
                    tools: vec![],
                    max_tokens: 2048,
                    temperature: None,
                },
                &mut |_| {},
            )
            .await?;
        Ok(turn.text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_llm::chat::{AssistantTurn, StopReason, Usage};
    use forge_llm::tier_from_id;

    fn text_msg(role: Role, text: &str) -> ChatMessage {
        ChatMessage {
            role,
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    #[test]
    fn estimativa_e_chars_sobre_quatro() {
        let messages = vec![text_msg(Role::User, &"a".repeat(400))];
        assert_eq!(estimate_tokens(&messages), 100);
    }

    #[test]
    fn threshold_e_tier_gated() {
        let small = CompactionPolicy::for_tier(tier_from_id("claude-haiku-4-5"), 1000);
        let large = CompactionPolicy::for_tier(tier_from_id("claude-opus-4-8"), 1000);
        // 800 tokens: acima de 75% (small compacta), abaixo de 90% (large não).
        let messages = vec![text_msg(Role::User, &"a".repeat(3200))];
        assert!(small.needs_compaction(&messages));
        assert!(!large.needs_compaction(&messages));
    }

    #[test]
    fn fronteira_segura_exige_assistente_sem_tool_use() {
        let safe = vec![
            text_msg(Role::User, "oi"),
            text_msg(Role::Assistant, "pronto"),
        ];
        assert!(CompactionPolicy::is_safe_boundary(&safe));

        let pending_tool = vec![ChatMessage {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "t1".into(),
                name: "read".into(),
                input: serde_json::json!({}),
            }],
        }];
        assert!(!CompactionPolicy::is_safe_boundary(&pending_tool));

        let user_last = vec![text_msg(Role::User, "oi")];
        assert!(!CompactionPolicy::is_safe_boundary(&user_last));
    }

    struct SummaryGen;
    impl Generator for SummaryGen {
        async fn generate(
            &self,
            req: GenerateRequest,
            _on_delta: &mut (dyn FnMut(&str) + Send),
        ) -> Result<AssistantTurn, GatewayError> {
            assert!(req.tools.is_empty(), "resumo não usa ferramentas");
            Ok(AssistantTurn {
                content: vec![ContentBlock::Text {
                    text: "resumo da conversa".into(),
                }],
                stop_reason: StopReason::EndTurn,
                usage: Usage::default(),
                provider: "test".into(),
            })
        }
    }

    #[tokio::test]
    async fn resumo_vem_do_gerador_sem_ferramentas() {
        let policy = CompactionPolicy::for_tier(tier_from_id("m"), 1000);
        let messages = vec![text_msg(Role::User, "tarefa")];
        let summary = policy.summarize(&SummaryGen, "m", &messages).await.unwrap();
        assert_eq!(summary, "resumo da conversa");
    }
}
