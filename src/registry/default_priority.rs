use crate::providers::providers::ProviderIdentifier;

pub(crate) fn default_priority(provider_id: ProviderIdentifier) -> u8 {
    match provider_id {
        ProviderIdentifier::Ollama => 20,
        ProviderIdentifier::OpenAI => 10,
    }
}
