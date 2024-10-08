use nu_ansi_term::Color;
use strum::IntoEnumIterator;
use table::{IntoRow, IntoTable, Row, Table};
mod table;

use crate::{
    providers::providers::ProviderIdentifier, registry::registry::Registry, ListArgs, ListObject,
    ListingFormat,
};

use crate::ColorMode;

use crate::die;

#[derive(serde::Serialize)]
struct Model {
    model_id: String,
    context: Option<u64>,
}

impl From<Vec<Model>> for Table {
    fn from(value: Vec<Model>) -> Self {
        let mut tab = Table::new();

        tab.set_header(standard_header(vec!["MODEL", "CONTEXT"]));

        for model in value {
            tab.add_row(standard_body(vec![
                model.model_id,
                match model.context {
                    Some(context) => context.to_string(),
                    None => "unknown".to_string(),
                },
            ]));
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

fn standard_header<R: IntoRow>(v: R) -> Row {
    let row = v.into_row();

    row.with_style(Color::Green.into())
}

fn standard_body<R: IntoRow>(v: R) -> Row {
    let row = v.into_row();

    row.with_style(Color::White.into())
}

impl From<Vec<ProvidedModel>> for Table {
    fn from(value: Vec<ProvidedModel>) -> Self {
        let mut tab = Table::new();

        tab.set_header(standard_header(vec!["MODEL", "PROVIDER", "CONTEXT"]));

        for model in value {
            tab.add_row(standard_body(vec![
                model.model_id,
                model.provider.to_string(),
                match model.context {
                    Some(context) => context.to_string(),
                    None => "unknown".to_string(),
                },
            ]));
        }

        tab
    }
}

#[derive(serde::Serialize)]
struct Provider {
    provider: ProviderIdentifier,
    priority: u8,
    activated: bool,
}

impl Into<Table> for Vec<Provider> {
    fn into(self) -> Table {
        let mut tab = Table::new();

        tab.set_header(standard_header(vec!["PROVIDER", "PRIORITY", "ACTIVATED"]));

        for provider in self {
            tab.add_row(standard_body(vec![
                provider.provider.to_string(),
                provider.priority.to_string(),
                if provider.activated {
                    "yes".to_string()
                } else {
                    "no".to_string()
                },
            ]));
        }

        tab
    }
}

fn get_providers(registry: &Registry) -> Vec<Provider> {
    let mut providers = Vec::new();

    for id in ProviderIdentifier::iter() {
        let provider = registry.provider(id);

        let priority = registry.priority(id);

        providers.push(Provider {
            provider: id,
            priority,
            activated: provider.is_some(),
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
            die!(
                "failed to list models: provider \"{0}\" is not activated",
                id
            );
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

fn format_output<O: IntoTable + serde::Serialize>(
    object: O,
    format: ListingFormat,
    color: ColorMode,
) {
    match format {
        ListingFormat::Json => {
            let output = serde_json::to_string_pretty(&object).expect("failed to seralize object");

            println!("{}", output);
        }
        ListingFormat::Table => {
            let mut tab = object.into_table();

            if matches!(color, ColorMode::Off) {
                tab.set_color(false);
            }

            print!("{}", tab);
        }
        ListingFormat::HeaderlessTable => {
            let mut tab = object.into_table();

            if matches!(color, ColorMode::Off) {
                tab.set_color(false);
            }

            tab.print_header(false);

            print!("{}", tab);
        }
    }
}

pub(crate) async fn list_cmd(color: ColorMode, registry: Registry, args: &ListArgs) {
    let format = args.format;

    match &args.object {
        ListObject::Models(args) => {
            if let Some(id) = args.provider {
                let models = get_models_for_provider(&registry, id).await;
                format_output(models, format, color);
            } else {
                let models = get_registered_models(&registry).await;
                format_output(models, format, color);
            }
        }
        ListObject::Providers => {
            let providers = get_providers(&registry);
            format_output(providers, format, color);
        }
    }
}
