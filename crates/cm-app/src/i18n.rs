//! UI strings through Fluent. English is embedded; more languages can be
//! added as `.ftl` files without code changes.

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use std::fmt;

const EN_US: &str = include_str!("../i18n/en-US.ftl");

pub struct I18n {
    bundle: FluentBundle<FluentResource>,
}

impl fmt::Debug for I18n {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("I18n")
    }
}

impl I18n {
    /// The built-in English strings.
    pub fn english() -> Self {
        let resource =
            FluentResource::try_new(EN_US.to_owned()).expect("en-US.ftl is checked by tests");
        let lang = "en-US".parse().expect("valid language id");
        let mut bundle = FluentBundle::new(vec![lang]);
        // No Unicode isolation marks: the embedded font has no glyphs for them.
        bundle.set_use_isolating(false);
        bundle
            .add_resource(resource)
            .expect("en-US.ftl has no duplicate ids");
        Self { bundle }
    }

    /// The message `id`. A missing id is shown as itself (and fails in debug builds).
    pub fn tr(&self, id: &str) -> String {
        self.format(id, None).unwrap_or_else(|| missing(id))
    }

    /// The message `id` with arguments.
    pub fn tr_args(&self, id: &str, args: &[(&str, FluentValue<'_>)]) -> String {
        let mut fluent_args = FluentArgs::new();
        for (name, value) in args {
            fluent_args.set(*name, value.clone());
        }
        self.format(id, Some(&fluent_args))
            .unwrap_or_else(|| missing(id))
    }

    /// The message `id` if it exists, else `fallback` (for data such as rhythm names).
    pub fn tr_or(&self, id: &str, fallback: &str) -> String {
        self.format(id, None).unwrap_or_else(|| fallback.to_owned())
    }

    /// Attribute `attr` of message `id` if both exist (e.g. `.short`).
    pub fn attr(&self, id: &str, attr: &str) -> Option<String> {
        let pattern = self.bundle.get_message(id)?.get_attribute(attr)?.value();
        let mut errors = Vec::new();
        let text = self.bundle.format_pattern(pattern, None, &mut errors);
        Some(text.into_owned())
    }

    fn format(&self, id: &str, args: Option<&FluentArgs<'_>>) -> Option<String> {
        let pattern = self.bundle.get_message(id)?.value()?;
        let mut errors = Vec::new();
        let text = self.bundle.format_pattern(pattern, args, &mut errors);
        debug_assert!(errors.is_empty(), "Fluent errors in {id}: {errors:?}");
        Some(text.into_owned())
    }
}

fn missing(id: &str) -> String {
    debug_assert!(false, "missing UI string {id}");
    id.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cm_core::{Rhythm, Subdivision, TempoMarking};

    fn has(i18n: &I18n, id: &str) -> bool {
        i18n.bundle
            .get_message(id)
            .and_then(|m| m.value())
            .is_some()
    }

    #[test]
    fn english_parses_without_errors() {
        let (_, errors) = match FluentResource::try_new(EN_US.to_owned()) {
            Ok(r) => (r, Vec::new()),
            Err((r, e)) => (r, e),
        };
        assert!(errors.is_empty(), "{errors:?}");
    }

    #[test]
    fn every_model_id_has_a_string() {
        let i18n = I18n::english();
        let mut ids: Vec<String> = Vec::new();
        ids.extend(Subdivision::ALL.map(|s| format!("subdivision-{}", s.id())));
        ids.extend(TempoMarking::ALL.map(|m| format!("tempo-{}", m.id())));
        for r in Rhythm::builtin() {
            ids.push(format!("rhythm-{}", r.id));
            ids.push(format!("rhythm-category-{}", r.category.id()));
        }
        for id in ids {
            assert!(has(&i18n, &id), "missing {id}");
        }
    }

    #[test]
    fn arguments_and_selectors() {
        let i18n = I18n::english();
        let text = i18n.tr_args(
            "beat-label",
            &[("number", 3.into()), ("level", "accent".into())],
        );
        assert_eq!(text, "Beat 3: accent");
        assert_eq!(i18n.tr_or("rhythm-unknown", "Fallback"), "Fallback");
        assert_eq!(
            i18n.attr("rhythm-son-clave-3-2", "short").as_deref(),
            Some("Son 3-2")
        );
        assert_eq!(i18n.attr("rhythm-samba", "short"), None);
    }
}
