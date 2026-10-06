//! The checks that look across keys: the start-up errors and the warnings
//! at the end of spec 13.3.
//!
//! Each key has already passed its own checks in [`crate::validate`]. These
//! look at how the keys fit together, so a config that contradicts itself is
//! an error and not a surprise later.

use crate::model::{Locality, Role, SearchProvider};
use crate::reader::{Found, Place, Report, quoted_list, shown_name};
use crate::validate::{Draft, ModelDraft, SearchDraft};

/// Runs every cross-check, recording what it finds in `report`.
pub(crate) fn cross_check(draft: &Draft, report: &mut Report) {
    let model_ids = draft
        .models
        .iter()
        .map(|model| (&model.place, model.id.as_ref()));
    duplicate_ids(model_ids, "model", report);
    let server_ids = draft
        .mcp_servers
        .iter()
        .map(|server| (&server.place, server.id.as_ref()));
    duplicate_ids(server_ids, "MCP server", report);

    for model in &draft.models {
        model_rules(model, report);
    }
    if let Some(search) = &draft.search {
        search_rules(search, report);
    }
    default_rules(draft, report);
    allow_cloud_rule(draft, report);
}

impl ModelDraft {
    /// Where the model runs, once its provider says or implies it. Hosted
    /// providers are always cloud.
    fn runs(&self) -> Option<Locality> {
        if self.provider?.is_hosted() {
            Some(Locality::Cloud)
        } else {
            self.locality.map(|found| found.value)
        }
    }

    /// Whether `allow_cloud = false` rules the model out. It allows only
    /// local brains and LAN brains marked `trusted_for_private`.
    fn needs_cloud_allowed(&self) -> bool {
        match self.runs() {
            Some(Locality::Cloud) => true,
            Some(Locality::Lan) => !self.trusted_for_private,
            Some(Locality::Local) | None => false,
        }
    }

    /// Whether the model may take a role. One with no `roles` line may take
    /// any.
    fn can_be(&self, role: Role) -> bool {
        self.roles
            .as_ref()
            .is_none_or(|roles| roles.contains(&role))
    }

    /// The model's id in quotes, or plain words when the id is too long or
    /// too odd to show.
    fn name(&self) -> String {
        self.id
            .as_ref()
            .and_then(|id| shown_name(&id.value))
            .map_or_else(
                || "that model".to_owned(),
                |id| format!("the model \"{id}\""),
            )
    }
}

/// Two models, or two MCP servers, can't share an `id`.
fn duplicate_ids<'d>(
    entries: impl Iterator<Item = (&'d Place, Option<&'d Found<String>>)>,
    what: &str,
    report: &mut Report,
) {
    let mut seen: Vec<(&str, usize)> = Vec::new();
    for (index, (place, id)) in entries.enumerate() {
        let Some(id) = id else {
            continue;
        };
        match seen.iter().find(|(seen_id, _)| *seen_id == id.value) {
            Some((_, first)) => report.error(
                Some(id.line),
                place.key("id"),
                format!("entry {first} already uses this id. Give each {what} its own."),
            ),
            None => seen.push((&id.value, index + 1)),
        }
    }
}

/// What one model's keys say about each other.
fn model_rules(model: &ModelDraft, report: &mut Report) {
    let Some(provider) = model.provider else {
        // Its provider is missing or wrong, which has already been reported.
        return;
    };
    let hosted = provider.is_hosted();
    let provider = provider.as_str();

    if hosted {
        if let Some(locality) = model
            .locality
            .filter(|found| found.value != Locality::Cloud)
        {
            report.error(
                Some(locality.line),
                model.place.key("locality"),
                format!(
                    "\"{provider}\" is a hosted provider, so its models always run in the cloud. Remove this line, or use the provider \"openai-compatible\" for a model you run yourself."
                ),
            );
        }
        if let Some(line) = model.endpoint_line {
            report.error(
                Some(line),
                model.place.key("endpoint"),
                format!(
                    "\"{provider}\" is a hosted provider, and Wrybill already knows its address. Remove this line. To use a server at an address of your own, set the provider to \"openai-compatible\"."
                ),
            );
        }
    }

    let runs = model.runs();
    if let Some(line) = model.trusted_for_private_line {
        match runs {
            Some(Locality::Cloud) if model.trusted_for_private => report.error(
                Some(line),
                model.place.key("trusted_for_private"),
                "only a LAN model can be trusted for private work, and this one runs in the cloud. Remove this line.",
            ),
            Some(Locality::Local) => report.warning(
                Some(line),
                model.place.key("trusted_for_private"),
                "isn't needed on a local model. Local models can always be used for private work.",
            ),
            _ => {}
        }
    }
    if let Some(line) = model.min_free_ram_gb_line {
        let elsewhere = match runs {
            Some(Locality::Cloud) => Some("cloud"),
            Some(Locality::Lan) => Some("LAN"),
            Some(Locality::Local) | None => None,
        };
        if let Some(elsewhere) = elsewhere {
            report.warning(
                Some(line),
                model.place.key("min_free_ram_gb"),
                format!(
                    "only matters for a model on this computer, so it has no effect on this {elsewhere} one."
                ),
            );
        }
    }
}

/// How the `[search]` keys fit the provider.
fn search_rules(search: &SearchDraft, report: &mut Report) {
    let place = Place::Section("search");

    let Some(provider) = search.provider else {
        if !search.has_provider && search.has_other_keys {
            report.error(
                Some(search.line),
                place.name(),
                format!(
                    "has other keys but no `provider`. Add `provider`, set to {}.",
                    quoted_list(SearchProvider::CHOICES.iter().map(|(word, _)| *word))
                ),
            );
        }
        return;
    };

    if provider.takes_a_key() {
        if let Some(line) = search.endpoint_line {
            report.error(
                Some(line),
                place.key("endpoint"),
                format!(
                    "only \"searxng\" takes an `endpoint`. Wrybill already knows the address of \"{}\". Remove this line.",
                    provider.as_str()
                ),
            );
        }
    } else if let Some(line) = search.api_key_line {
        report.error(
            Some(line),
            place.key("api_key"),
            "\"searxng\" takes no key. Remove this line.",
        );
    }
}

/// Each `[defaults]` entry has to name a model that can do that job.
fn default_rules(draft: &Draft, report: &mut Report) {
    let entries = [
        ("planner", Role::Planner, &draft.defaults.planner),
        ("worker", Role::Worker, &draft.defaults.worker),
        ("summariser", Role::Summariser, &draft.defaults.summariser),
    ];
    for (key, role, entry) in entries {
        let Some(entry) = entry else {
            continue;
        };
        let place = Place::Section("defaults").key(key);
        let named = draft
            .models
            .iter()
            .find(|model| model.id.as_ref().is_some_and(|id| id.value == entry.value));
        let Some(model) = named else {
            report.error(Some(entry.line), place, no_such_model(&entry.value, draft));
            continue;
        };

        let name = model.name();
        if !model.enabled {
            report.error(
                Some(entry.line),
                place.clone(),
                format!(
                    "{name} is switched off with `enabled = false`. Switch it on, or name another model."
                ),
            );
        }
        if !model.can_be(role) {
            report.error(
                Some(entry.line),
                place.clone(),
                format!(
                    "{name} lists its roles, and \"{}\" isn't one of them. Add it to that model's `roles`, or name another model.",
                    role.as_str()
                ),
            );
        }
        if !draft.routing.allow_cloud && model.needs_cloud_allowed() {
            let why = if model.runs() == Some(Locality::Lan) {
                "is a LAN model that isn't marked `trusted_for_private`"
            } else {
                "runs in the cloud"
            };
            report.error(
                Some(entry.line),
                place,
                format!("{name} {why}, and `allow_cloud = false` rules that out."),
            );
        }
    }
}

/// What to say when a `[defaults]` entry names a model that isn't there.
fn no_such_model(wanted: &str, draft: &Draft) -> String {
    let missing = match shown_name(wanted) {
        Some(id) => format!("no model has the id \"{id}\"."),
        None => "no model has that id.".to_owned(),
    };
    if draft.models.is_empty() {
        return format!("{missing} This file has no models yet.");
    }

    // The ids that can be shown, each one once.
    let mut ids: Vec<&str> = Vec::new();
    for model in &draft.models {
        let id = model.id.as_ref().and_then(|id| shown_name(&id.value));
        if let Some(id) = id.filter(|id| !ids.contains(id)) {
            ids.push(id);
        }
    }
    match ids.as_slice() {
        [] => missing,
        [only] => format!("{missing} The only model here is \"{only}\"."),
        [first @ .., last] if ids.len() <= 8 => {
            let first: Vec<String> = first.iter().map(|id| format!("\"{id}\"")).collect();
            format!(
                "{missing} The models here are {} and \"{last}\".",
                first.join(", ")
            )
        }
        _ => missing,
    }
}

/// `allow_cloud = false` can't leave Wrybill with nothing to plan with.
fn allow_cloud_rule(draft: &Draft, report: &mut Report) {
    if draft.routing.allow_cloud {
        return;
    }
    let mut planners = draft
        .models
        .iter()
        .filter(|model| model.enabled && model.can_be(Role::Planner))
        .peekable();
    // No model that can plan isn't a contradiction: it's the first-run state.
    if planners.peek().is_none() {
        return;
    }
    if planners.all(ModelDraft::needs_cloud_allowed) {
        report.error(
            draft.allow_cloud_line,
            Place::Section("routing").key("allow_cloud"),
            "leaves no model that can plan. Every model that could is a cloud model, or a LAN model that isn't marked `trusted_for_private`. Add a local model, mark a LAN model as trusted, or set `allow_cloud = true`.",
        );
    }
}
