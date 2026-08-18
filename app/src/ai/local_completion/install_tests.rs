//! The runtime wiring: does a key or a settings edit actually reach the
//! process global the four call sites read?
//!
//! Everything else in this module is a pure function. This is the part that is
//! only true if the subscriptions are right, and it is the part that would fail
//! silently — a missed subscription looks like a feature that works but needs a
//! restart, which is the kind of thing nobody reports as a bug.

use ai::api_keys::{ApiKeyManager, CustomEndpointParams, CustomEndpointSchema};
use ai::llm_provider::LLMProvider;
use settings::Setting as _;
use warpui::{App, SingletonEntity as _};

use super::config::{self, Unconfigured};
use crate::settings::{LocalAiFeature, LocalAiSettings};
use crate::test_util::settings::initialize_settings_for_tests;

/// One test rather than three, because `config::current` reads a process
/// global: separate tests would run in parallel in the same binary and clobber
/// each other's state.
#[test]
fn keys_and_settings_reach_the_call_sites_without_a_restart() {
    App::test((), |mut app| async move {
        initialize_settings_for_tests(&mut app);
        // Pasting a provider key emits a telemetry event on the way past.
        app.update(warp_core::telemetry::testing::MockTelemetryContextProvider::register);
        app.update(config::install);

        // Nothing configured: an error naming what to do, not a silent default
        // that would let a request leave for somewhere the user never chose.
        assert_eq!(
            config::current().unwrap_err(),
            Unconfigured::NothingConfigured
        );

        // A key pasted into Settings emits `KeysUpdated`. Without a
        // subscription to it, the feature would stay dead until the next
        // launch.
        app.update(|ctx| {
            ApiKeyManager::handle(ctx).update(ctx, |manager, ctx| {
                manager.set_provider_key(
                    LLMProvider::Anthropic,
                    Some("sk-ant-test".to_owned()),
                    ctx,
                );
            });
        });

        let resolved = config::current().expect("a pasted key is enough on its own");
        assert_eq!(resolved.api_key, "sk-ant-test");
        assert_eq!(resolved.schema, CustomEndpointSchema::AnthropicMessages);

        // A Custom Inference endpoint outranks the pasted key.
        app.update(|ctx| {
            ApiKeyManager::handle(ctx).update(ctx, |manager, ctx| {
                manager.add_custom_endpoint(
                    CustomEndpointParams {
                        name: "local".to_owned(),
                        url: "http://127.0.0.1:11434/v1/chat/completions".to_owned(),
                        api_key: "local-key".to_owned(),
                        models: vec![("qwen2.5-coder".to_owned(), None, None)],
                        schema: CustomEndpointSchema::OpenaiChatCompletions,
                    },
                    ctx,
                );
            });
        });

        let resolved = config::current().unwrap();
        assert_eq!(
            resolved.endpoint,
            "http://127.0.0.1:11434/v1/chat/completions"
        );
        assert_eq!(
            resolved.model_for(LocalAiFeature::CodeReview),
            "qwen2.5-coder"
        );

        // Settings groups *emit* their changed event without calling `notify`,
        // so `observe` would never fire here — this asserts the subscription is
        // the right one.
        app.update(|ctx| {
            LocalAiSettings::handle(ctx).update(ctx, |settings, ctx| {
                settings
                    .local_ai_model_code_review
                    .set_value("bigger-model".to_owned(), ctx)
                    .unwrap();
            });
        });

        let resolved = config::current().unwrap();
        assert_eq!(
            resolved.model_for(LocalAiFeature::CodeReview),
            "bigger-model"
        );
        assert_eq!(
            resolved.model_for(LocalAiFeature::BlockTitle),
            "qwen2.5-coder",
            "a per-feature override must not leak into the other features"
        );
    });
}
