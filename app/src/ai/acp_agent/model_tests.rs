use agent_client_protocol::schema::v1::{SessionConfigSelectOption, SessionConfigValueId};

use super::*;

fn option(id: &str, category: Option<SessionConfigOptionCategory>) -> SessionConfigOption {
    let option = SessionConfigOption::select(
        SessionConfigId::from(id.to_owned()),
        id.to_owned(),
        SessionConfigValueId::from(format!("{id}-current")),
        vec![
            SessionConfigSelectOption::new("a", "A"),
            SessionConfigSelectOption::new("b", "B"),
        ],
    );
    match category {
        Some(category) => option.category(category),
        None => option,
    }
}

fn model_option(id: &str) -> SessionConfigOption {
    option(id, Some(SessionConfigOptionCategory::Model))
}

/// **The finding, as a test.** A model option the agent advertised survives
/// the filter; anything the filter rejected is absent from the render door,
/// which is the only list a surface may show.
#[test]
fn a_model_option_survives_the_filter() {
    let catalog = Catalog::of(Some(&[
        model_option("model"),
        option("mode", Some(SessionConfigOptionCategory::Mode)),
        option("agent", None),
    ]));

    assert_eq!(
        catalog
            .options()
            .iter()
            .map(|o| o.id.0.to_string())
            .collect::<Vec<_>>(),
        vec!["model".to_owned()],
        "only the category-less rule's one denial differs from `Mode`'s, and here they agree"
    );
}

/// **The survey's one live case.** Measured 2026-08-30, `claude-agent-acp` ships
/// an option (`id: "agent"`) with **no category at all**. The
/// unknown-must-not-qualify rule was written for a category this build had not
/// read; the measured case is a category that is absent. Same answer: a
/// `None` is not `Some(Model)`.
#[test]
fn an_option_with_no_category_is_not_rendered() {
    let catalog = Catalog::of(Some(&[option("agent", None)]));

    assert!(
        catalog.options().is_empty(),
        "an unlabelled option must not leak into the model picker"
    );
}

/// `ModelConfig` exits the enum for a reason this seam exists not to cross:
/// it is the knob *behind* the selector, not the selector. `Mode`,
/// `ThoughtLevel` and an unknown `Other(..)` all fail the same allowlist
/// check the tool kinds do.
#[test]
fn mode_model_config_thought_level_and_unknown_categories_are_not_rendered() {
    for category in [
        SessionConfigOptionCategory::Mode,
        SessionConfigOptionCategory::ModelConfig,
        SessionConfigOptionCategory::ThoughtLevel,
        SessionConfigOptionCategory::Other("_custom".to_owned()),
    ] {
        let described = format!("{category:?}");
        let catalog = Catalog::of(Some(&[option("x", Some(category))]));
        assert!(
            catalog.options().is_empty(),
            "{described} must not qualify for the model picker"
        );
    }
}

/// `None` and an empty list are the same case: no model was claimed, so there
/// is nothing to show — and nothing to send either, since both doors share
/// the one filtered value.
#[test]
fn an_agent_that_advertised_nothing_yields_an_empty_catalog() {
    assert!(Catalog::of(None).options().is_empty());
    assert!(Catalog::of(Some(&[])).options().is_empty());
}

/// **The send door shares the render door's filter.** A id the catalog holds
/// — a `category: "model"` option — gets a write, carrying the value back
/// verbatim. An id the filter rejected (`mode`) and a id never advertised get
/// `None`, so a caller cannot conjure a write the render could never have
/// shown: the `switch_mode`-through-a-generic-picker hazard, from the write
/// side.
#[test]
fn a_request_is_built_only_for_a_config_id_the_catalog_holds() {
    let catalog = Catalog::of(Some(&[
        model_option("model"),
        option("mode", Some(SessionConfigOptionCategory::Mode)),
    ]));
    let session_id = SessionId::from("session-1".to_owned());
    let model_id = SessionConfigId::from("model".to_owned());
    let mode_id = SessionConfigId::from("mode".to_owned());
    let value = SessionConfigOptionValue::value_id("model-b");

    let request = catalog
        .request(&session_id, &model_id, value.clone())
        .expect("a held model config id is sendable");
    assert_eq!(request.session_id, session_id);
    assert_eq!(request.config_id, model_id);
    assert_eq!(request.value, value, "the value passes back verbatim");

    assert_eq!(
        catalog.request(&session_id, &mode_id, value.clone()),
        None,
        "a non-model config id is not sendable, exactly as it is not renderable"
    );
    assert_eq!(
        catalog.request(
            &session_id,
            &SessionConfigId::from("hallucinated".to_owned()),
            value
        ),
        None,
        "a config id the agent never advertised is not sendable either"
    );
}

/// The gate holds even when the catalog is empty: no advertised model means no
/// write exists, for any id, including ones that would qualify on category.
#[test]
fn an_empty_catalog_can_send_nothing() {
    let catalog = Catalog::of(Some(&[]));
    let session_id = SessionId::from("session-1".to_owned());

    assert_eq!(
        catalog.request(
            &session_id,
            &SessionConfigId::from("model".to_owned()),
            SessionConfigOptionValue::value_id("anything")
        ),
        None
    );
}
