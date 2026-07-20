//! Tauri command for single-meeting Q&A: retrieve grounding excerpts, ask the
//! configured LLM, and return the answer with the transcript segments it cited.

use serde::Serialize;
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager, Runtime};

use super::retrieval::{self, QaSegment, DEFAULT_CONTEXT_BUDGET_CHARS};
use crate::database::repositories::{meeting::MeetingsRepository, setting::SettingsRepository};
use crate::state::AppState;
use crate::summary::llm_client::{generate_summary, LLMProvider};

/// A transcript segment the answer was grounded on, surfaced to the UI so the
/// user can jump to that moment.
#[derive(Debug, Serialize)]
pub struct AnswerSource {
    pub number: usize,
    #[serde(rename = "segmentId")]
    pub segment_id: String,
    pub timestamp: String,
    pub text: String,
}

/// Result of asking a question about a meeting.
#[derive(Debug, Serialize)]
pub struct MeetingAnswer {
    pub answer: String,
    pub sources: Vec<AnswerSource>,
    pub provider: String,
    pub model: String,
    #[serde(rename = "excerptsUsed")]
    pub excerpts_used: usize,
}

/// Everything needed to make one LLM call, resolved from stored settings.
struct LlmCallConfig {
    provider: LLMProvider,
    provider_label: String,
    model: String,
    api_key: String,
    ollama_endpoint: Option<String>,
    custom_openai_endpoint: Option<String>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    top_p: Option<f32>,
}

/// Resolve the configured summarization provider/model/credentials the same way
/// the summary pipeline does, so Q&A uses whatever the user already set up.
async fn resolve_llm_config(pool: &SqlitePool) -> Result<LlmCallConfig, String> {
    let setting = SettingsRepository::get_model_config(pool)
        .await
        .map_err(|e| format!("Failed to read model configuration: {}", e))?
        .ok_or_else(|| "No AI model is configured. Set one in Settings first.".to_string())?;

    let provider_label = setting.provider;
    let provider = LLMProvider::from_str(&provider_label)?;
    let mut model = setting.model;

    let mut ollama_endpoint = None;
    let mut custom_openai_endpoint = None;
    let mut max_tokens = None;
    let mut temperature = None;
    let mut top_p = None;
    let api_key: String;

    if provider == LLMProvider::Ollama {
        api_key = String::new();
        ollama_endpoint = setting.ollama_endpoint;
    } else if provider == LLMProvider::BuiltInAI {
        api_key = String::new();
    } else if provider == LLMProvider::CustomOpenAI {
        let cfg = SettingsRepository::get_custom_openai_config(pool)
            .await
            .map_err(|e| format!("Failed to read custom OpenAI config: {}", e))?
            .ok_or_else(|| "Custom OpenAI provider selected but not configured.".to_string())?;
        custom_openai_endpoint = Some(cfg.endpoint);
        api_key = cfg.api_key.unwrap_or_default();
        if !cfg.model.trim().is_empty() {
            model = cfg.model;
        }
        max_tokens = cfg.max_tokens.map(|t| t as u32);
        temperature = cfg.temperature;
        top_p = cfg.top_p;
    } else {
        // OpenAI, Claude, Groq, OpenRouter all require an API key.
        api_key = SettingsRepository::get_api_key(pool, &provider_label)
            .await
            .map_err(|e| format!("Failed to read API key: {}", e))?
            .filter(|k| !k.is_empty())
            .ok_or_else(|| format!("API key not found for {}.", provider_label))?;
    }

    if model.trim().is_empty() {
        return Err("No AI model name is configured.".to_string());
    }

    Ok(LlmCallConfig {
        provider,
        provider_label,
        model,
        api_key,
        ollama_endpoint,
        custom_openai_endpoint,
        max_tokens,
        temperature,
        top_p,
    })
}

/// Answer a question about a single meeting using its transcript as grounding.
#[tauri::command]
pub async fn api_ask_meeting_question<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    question: String,
    _auth_token: Option<String>,
) -> Result<MeetingAnswer, String> {
    let question = question.trim().to_string();
    if question.is_empty() {
        return Err("Please enter a question.".to_string());
    }
    let meeting_id = meeting_id.trim();
    if meeting_id.is_empty() {
        return Err("meeting_id is required.".to_string());
    }

    let pool = state.db_manager.pool();

    // 1. Load the meeting and its transcript.
    let meeting = MeetingsRepository::get_meeting(pool, meeting_id)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => "Meeting not found.".to_string(),
            other => format!("Failed to load meeting: {}", other),
        })?
        .ok_or_else(|| "Meeting not found.".to_string())?;

    if meeting.transcripts.is_empty() {
        return Err("This meeting has no transcript to answer from.".to_string());
    }

    // 2. Retrieve the most relevant excerpts as grounding context.
    let segments: Vec<QaSegment> = meeting
        .transcripts
        .iter()
        .map(|t| QaSegment {
            id: t.id.clone(),
            timestamp: t.timestamp.clone(),
            audio_start_time: t.audio_start_time,
            text: t.text.clone(),
        })
        .collect();

    let excerpts = retrieval::select_excerpts(&segments, &question, DEFAULT_CONTEXT_BUDGET_CHARS);
    if excerpts.is_empty() {
        return Err("This meeting has no transcript text to answer from.".to_string());
    }

    // 3. Build the grounded prompt.
    let system_prompt = retrieval::build_system_prompt();
    let user_prompt = retrieval::build_user_prompt(&meeting.title, &excerpts, &question);

    // 4. Resolve the configured model and ask it.
    let config = resolve_llm_config(pool).await?;
    let app_data_dir = app.path().app_data_dir().ok();
    let client = reqwest::Client::new();

    let answer = generate_summary(
        &client,
        &config.provider,
        &config.model,
        &config.api_key,
        &system_prompt,
        &user_prompt,
        config.ollama_endpoint.as_deref(),
        config.custom_openai_endpoint.as_deref(),
        config.max_tokens,
        config.temperature,
        config.top_p,
        app_data_dir.as_ref(),
        None,
    )
    .await?;

    // 5. Map the model's citations back to transcript segments.
    let cited = retrieval::extract_citations(&answer, excerpts.len());
    let sources: Vec<AnswerSource> = excerpts
        .iter()
        .filter(|e| cited.contains(&e.number))
        .map(|e| AnswerSource {
            number: e.number,
            segment_id: e.segment_id.clone(),
            timestamp: e.timestamp.clone(),
            text: e.text.clone(),
        })
        .collect();

    Ok(MeetingAnswer {
        answer,
        sources,
        provider: config.provider_label,
        model: config.model,
        excerpts_used: excerpts.len(),
    })
}
