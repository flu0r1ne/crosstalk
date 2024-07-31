use strum::IntoEnumIterator;
use table::{IntoTable, Table};
mod table;

use crate::{
    providers::providers::ProviderIdentifier,
    registry::{populate::populated_registry, registry::Registry},
    ListArgs, ListObject, ListingFormat,
};

use die::die;

#[derive(serde::Serialize)]
struct Model {
    model_id: String,
    context: Option<u64>,
}

impl From<Vec<Model>> for Table {
    fn from(value: Vec<Model>) -> Self {
        let mut tab = Table::new();

        tab.set_header(vec!["MODEL", "CONTEXT"]);

        for model in value {
            tab.add_row(vec![
                model.model_id,
                match model.context {
                    Some(context) => context.to_string(),
                    None => "unknown".to_string(),
                },
            ]);
        }

        tab
    }
}

#[derive(serde::Serialize)]
struct ProvidedModel {
    model_id: String,
    provider: ProviderIdentifier,
    context: Option<u64>,
}

impl From<Vec<ProvidedModel>> for Table {
    fn from(value: Vec<ProvidedModel>) -> Self {
        let mut tab = Table::new();

        tab.set_header(vec!["MODEL", "PROVIDER", "CONTEXT"]);

        for model in value {
            tab.add_row(vec![
                model.model_id,
                model.provider.to_string(),
                match model.context {
                    Some(context) => context.to_string(),
                    None => "unknown".to_string(),
                },
            ]);
        }

        tab
    }
}

#[derive(serde::Serialize)]
struct Provider {
    provider: ProviderIdentifier,
    enabled: bool,
}

impl Into<Table> for Vec<Provider> {
    fn into(self) -> Table {
        let mut tab = Table::new();

        tab.set_header(vec!["PROVIDER", "ENABLED"]);

        for provider in self {
            tab.add_row(vec![
                provider.provider.to_string(),
                if provider.enabled {
                    "enabled".to_string()
                } else {
                    "disabled".to_string()
                },
            ]);
        }

        tab
    }
}

fn get_providers(registry: &Registry) -> Vec<Provider> {
    let mut providers = Vec::new();

    for id in ProviderIdentifier::iter() {
        let provider = registry.provider(id);

        providers.push(Provider {
            provider: id,
            enabled: provider.is_some(),
        });
    }

    providers
}

async fn get_registered_models(registry: &Registry) -> Vec<ProvidedModel> {
    match registry.registred_models().await {
        Ok(models) => {
            let registered_models: Vec<ProvidedModel> = models
                .into_iter()
                .map(|pm| ProvidedModel {
                    model_id: pm.model.id,
                    provider: pm.provider,
                    context: pm.model.context_length,
                })
                .collect();

            registered_models
        }
        Err(err) => {
            die!("failed to list models: {}", err);
        }
    }
}

async fn get_models_for_provider(registry: &Registry, id: ProviderIdentifier) -> Vec<Model> {
    let provider = match registry.provider(id) {
        Some(provider) => provider,
        None => {
            die!("failed to list models: provider \"{0}\" is not enabled", id);
        }
    };

    let models = match provider.models().await {
        Ok(models) => models,
        Err(err) => die!("failed to list models: {}", err),
    };

    let registered_models: Vec<Model> = models
        .into_iter()
        .map(|m| Model {
            model_id: m.id,
            context: m.context_length,
        })
        .collect();

    registered_models
}

fn format_output<O: IntoTable + serde::Serialize>(object: O, format: ListingFormat) {
    match format {
        ListingFormat::Json => {
            let output = serde_json::to_string_pretty(&object).expect("failed to seralize object");

            println!("{}", output);
        }
        ListingFormat::Table => {
            let tab = object.into_table();

            print!("{}", tab);
        }
        ListingFormat::HeaderlessTable => {
            let mut tab = object.into_table();

            tab.print_header(false);

            print!("{}", tab);
        }
    }
}

pub(crate) async fn list_cmd(args: &ListArgs) {
    let format = args.format;

    let registry = populated_registry().await;

    match &args.object {
        ListObject::Models(args) => {
            if let Some(id) = args.provider {
                let models = get_models_for_provider(&registry, id).await;
                format_output(models, format);
            } else {
                let models = get_registered_models(&registry).await;
                format_output(models, format);
            }
        }
        ListObject::Providers => {
            let providers = get_providers(&registry);
            format_output(providers, format);
        }
    }
}
