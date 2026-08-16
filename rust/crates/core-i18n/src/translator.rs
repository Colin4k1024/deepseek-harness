//! Translator — translates messages to the target locale.

use std::collections::HashMap;

use crate::locale::Locale;
use crate::message::{Message, MessageId};

/// Translation table for a single locale.
pub type TranslationTable = HashMap<MessageId, String>;

/// Translator — translates messages to the target locale.
pub struct Translator {
    /// Default locale.
    default_locale: Locale,

    /// Translation tables per locale.
    translations: HashMap<Locale, TranslationTable>,
}

impl Translator {
    /// Create a new translator with the default locale.
    pub fn new(default_locale: Locale) -> Self {
        Self {
            default_locale,
            translations: HashMap::new(),
        }
    }

    /// Add translations for a locale.
    pub fn add_translations(&mut self, locale: Locale, table: TranslationTable) {
        self.translations.insert(locale, table);
    }

    /// Translate a message to the target locale.
    ///
    /// Falls back to the default locale if translation is not found.
    /// Falls back to the message ID if no translation exists at all.
    pub fn translate(&self, message: &Message, locale: Locale) -> String {
        // Try target locale
        if let Some(table) = self.translations.get(&locale)
            && let Some(template) = table.get(&message.id)
        {
            return self.interpolate(template, &message.params);
        }

        // Fallback to default locale
        if locale != self.default_locale
            && let Some(table) = self.translations.get(&self.default_locale)
            && let Some(template) = table.get(&message.id)
        {
            return self.interpolate(template, &message.params);
        }

        // Final fallback: message ID as string
        format!("{:?}", message.id)
    }

    /// Interpolate parameters into a template string.
    ///
    /// Parameters are referenced as `{param_name}` in the template.
    fn interpolate(&self, template: &str, params: &HashMap<String, String>) -> String {
        let mut result = template.to_string();
        for (key, value) in params {
            result = result.replace(&format!("{{{key}}}"), value);
        }
        result
    }

    /// Get the default locale.
    pub fn default_locale(&self) -> Locale {
        self.default_locale
    }

    /// Check if a locale has translations.
    pub fn has_locale(&self, locale: Locale) -> bool {
        self.translations.contains_key(&locale)
    }
}

/// Create a translator with built-in English translations.
pub fn create_default_translator() -> Translator {
    let mut translator = Translator::new(Locale::En);

    let mut en = TranslationTable::new();
    en.insert(MessageId::InvalidApiKey, "Invalid API key".into());
    en.insert(MessageId::RateLimitExceeded, "Rate limit exceeded".into());
    en.insert(MessageId::QuotaExceeded, "Quota exceeded".into());
    en.insert(
        MessageId::SessionNotFound,
        "Session not found: {session_id}".into(),
    );
    en.insert(
        MessageId::ToolNotFound,
        "Tool not found: {tool_name}".into(),
    );
    en.insert(MessageId::LlmError, "LLM error: {error}".into());
    en.insert(MessageId::SandboxUnavailable, "Sandbox unavailable".into());
    en.insert(MessageId::InternalError, "Internal server error".into());
    en.insert(MessageId::AgentStarted, "Agent started".into());
    en.insert(MessageId::AgentCompleted, "Agent completed".into());
    en.insert(MessageId::AgentFailed, "Agent failed: {error}".into());
    en.insert(MessageId::Welcome, "Welcome to DeepSeek Harness".into());
    en.insert(MessageId::Success, "Success".into());
    en.insert(MessageId::Failure, "Failure".into());
    translator.add_translations(Locale::En, en);

    let mut zh_cn = TranslationTable::new();
    zh_cn.insert(MessageId::InvalidApiKey, "无效的 API 密钥".into());
    zh_cn.insert(MessageId::RateLimitExceeded, "请求频率超限".into());
    zh_cn.insert(MessageId::QuotaExceeded, "配额已用完".into());
    zh_cn.insert(
        MessageId::SessionNotFound,
        "会话未找到：{session_id}".into(),
    );
    zh_cn.insert(MessageId::ToolNotFound, "工具未找到：{tool_name}".into());
    zh_cn.insert(MessageId::LlmError, "大模型错误：{error}".into());
    zh_cn.insert(MessageId::SandboxUnavailable, "沙箱不可用".into());
    zh_cn.insert(MessageId::InternalError, "服务器内部错误".into());
    zh_cn.insert(MessageId::AgentStarted, "Agent 已启动".into());
    zh_cn.insert(MessageId::AgentCompleted, "Agent 已完成".into());
    zh_cn.insert(MessageId::AgentFailed, "Agent 失败：{error}".into());
    zh_cn.insert(MessageId::Welcome, "欢迎使用 DeepSeek Harness".into());
    zh_cn.insert(MessageId::Success, "成功".into());
    zh_cn.insert(MessageId::Failure, "失败".into());
    translator.add_translations(Locale::ZhCn, zh_cn);

    translator
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_english() {
        let translator = create_default_translator();
        let msg = Message::new(MessageId::Welcome);
        assert_eq!(
            translator.translate(&msg, Locale::En),
            "Welcome to DeepSeek Harness"
        );
    }

    #[test]
    fn translate_chinese() {
        let translator = create_default_translator();
        let msg = Message::new(MessageId::Welcome);
        assert_eq!(
            translator.translate(&msg, Locale::ZhCn),
            "欢迎使用 DeepSeek Harness"
        );
    }

    #[test]
    fn fallback_to_default_locale() {
        let translator = create_default_translator();
        let msg = Message::new(MessageId::Welcome);
        // Japanese not available, falls back to English
        assert_eq!(
            translator.translate(&msg, Locale::Ja),
            "Welcome to DeepSeek Harness"
        );
    }

    #[test]
    fn interpolation() {
        let translator = create_default_translator();
        let msg = Message::new(MessageId::SessionNotFound).with_param("session_id", "abc-123");
        assert_eq!(
            translator.translate(&msg, Locale::En),
            "Session not found: abc-123"
        );
    }

    #[test]
    fn interpolation_chinese() {
        let translator = create_default_translator();
        let msg = Message::new(MessageId::LlmError).with_param("error", "timeout");
        assert_eq!(
            translator.translate(&msg, Locale::ZhCn),
            "大模型错误：timeout"
        );
    }

    #[test]
    fn has_locale() {
        let translator = create_default_translator();
        assert!(translator.has_locale(Locale::En));
        assert!(translator.has_locale(Locale::ZhCn));
        assert!(!translator.has_locale(Locale::Ja));
    }
}
