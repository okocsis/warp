#[cfg(not(target_family = "wasm"))]
use ai::api_keys::ApiKeyManager;
#[cfg(not(target_family = "wasm"))]
use ai::codex_subscription::oauth::TokenResponse;
#[cfg(not(target_family = "wasm"))]
use base64::Engine as _;
#[cfg(not(target_family = "wasm"))]
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
#[cfg(not(target_family = "wasm"))]
use uuid::Uuid;
#[cfg(not(target_family = "wasm"))]
use warpui::App;

use super::{
    AgentAttributionToggleState, ChatGptSubscriptionButtonAction, GrokSubscriptionButtonAction,
    chatgpt_subscription_button_action, derive_agent_attribution_toggle_state,
    grok_subscription_button_action, should_render_chatgpt_subscription,
    subscription_controls_enabled,
};
#[cfg(not(target_family = "wasm"))]
use super::{chatgpt_oauth_attempt_is_current, take_chatgpt_tokens_for_disconnect};
use crate::workspaces::workspace::AdminEnablementSetting;

#[test]
fn respect_user_setting_returns_user_pref_unlocked() {
    let state = derive_agent_attribution_toggle_state(
        &AdminEnablementSetting::RespectUserSetting,
        true,
        true,
    );
    assert_eq!(
        state,
        AgentAttributionToggleState {
            is_enabled: true,
            is_forced_by_org: false,
            is_disabled: false,
        }
    );
}

#[test]
fn respect_user_setting_with_user_off_returns_unchecked_unlocked() {
    let state = derive_agent_attribution_toggle_state(
        &AdminEnablementSetting::RespectUserSetting,
        false,
        true,
    );
    assert_eq!(
        state,
        AgentAttributionToggleState {
            is_enabled: false,
            is_forced_by_org: false,
            is_disabled: false,
        }
    );
}

#[test]
fn team_enable_locks_toggle_on_regardless_of_user_pref() {
    let state = derive_agent_attribution_toggle_state(&AdminEnablementSetting::Enable, false, true);
    assert_eq!(
        state,
        AgentAttributionToggleState {
            is_enabled: true,
            is_forced_by_org: true,
            is_disabled: true,
        }
    );
}

#[test]
fn team_disable_locks_toggle_off_regardless_of_user_pref() {
    let state = derive_agent_attribution_toggle_state(&AdminEnablementSetting::Disable, true, true);
    assert_eq!(
        state,
        AgentAttributionToggleState {
            is_enabled: false,
            is_forced_by_org: true,
            is_disabled: true,
        }
    );
}

#[test]
fn ai_globally_disabled_marks_toggle_disabled_but_not_forced() {
    let state = derive_agent_attribution_toggle_state(
        &AdminEnablementSetting::RespectUserSetting,
        true,
        false,
    );
    assert_eq!(
        state,
        AgentAttributionToggleState {
            is_enabled: true,
            is_forced_by_org: false,
            is_disabled: true,
        }
    );
}

#[test]
fn team_force_takes_precedence_over_global_ai_disabled() {
    let state =
        derive_agent_attribution_toggle_state(&AdminEnablementSetting::Enable, false, false);
    assert_eq!(
        state,
        AgentAttributionToggleState {
            is_enabled: true,
            is_forced_by_org: true,
            is_disabled: true,
        }
    );
}

#[test]
fn grok_button_action_reflects_tokens_and_attempt_phase() {
    assert_eq!(
        grok_subscription_button_action(false, None),
        GrokSubscriptionButtonAction::Connect
    );
    assert_eq!(
        grok_subscription_button_action(false, Some(false)),
        GrokSubscriptionButtonAction::Cancel
    );
    assert_eq!(
        grok_subscription_button_action(false, Some(true)),
        GrokSubscriptionButtonAction::Cancelling
    );
    // Stored tokens take precedence regardless of attempt phase.
    assert_eq!(
        grok_subscription_button_action(true, None),
        GrokSubscriptionButtonAction::Disconnect
    );
    assert_eq!(
        grok_subscription_button_action(true, Some(false)),
        GrokSubscriptionButtonAction::Disconnect
    );
    assert_eq!(
        grok_subscription_button_action(true, Some(true)),
        GrokSubscriptionButtonAction::Disconnect
    );
}

#[test]
fn chatgpt_button_action_reflects_credentials_and_attempt_phase() {
    assert_eq!(
        chatgpt_subscription_button_action(false, None),
        ChatGptSubscriptionButtonAction::Connect
    );
    assert_eq!(
        chatgpt_subscription_button_action(false, Some(false)),
        ChatGptSubscriptionButtonAction::Cancel
    );
    assert_eq!(
        chatgpt_subscription_button_action(false, Some(true)),
        ChatGptSubscriptionButtonAction::Cancelling
    );
    assert_eq!(
        chatgpt_subscription_button_action(true, None),
        ChatGptSubscriptionButtonAction::Disconnect
    );
    assert_eq!(
        chatgpt_subscription_button_action(true, Some(false)),
        ChatGptSubscriptionButtonAction::Disconnect
    );
}

#[test]
fn chatgpt_subscription_visibility_requires_feature_and_provider_keys() {
    assert!(should_render_chatgpt_subscription(true, true));
    assert!(!should_render_chatgpt_subscription(false, true));
    assert!(!should_render_chatgpt_subscription(true, false));
}

#[test]
fn subscription_controls_require_ai_byo_and_team_policy() {
    assert!(subscription_controls_enabled(true, true, true));
    assert!(!subscription_controls_enabled(false, true, true));
    assert!(!subscription_controls_enabled(true, false, true));
    assert!(!subscription_controls_enabled(true, true, false));
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn stale_chatgpt_oauth_attempts_cannot_complete() {
    let active_id = Uuid::new_v4();

    assert!(chatgpt_oauth_attempt_is_current(Some(active_id), active_id));
    assert!(!chatgpt_oauth_attempt_is_current(None, active_id));
    assert!(!chatgpt_oauth_attempt_is_current(
        Some(Uuid::new_v4()),
        active_id
    ));
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn chatgpt_disconnect_clears_tokens_before_revocation() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            warpui_extras::secure_storage::register_noop("test", ctx);
        });
        let manager = app.add_singleton_model(ApiKeyManager::new);
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
        let payload = URL_SAFE_NO_PAD
            .encode(br#"{"https://api.openai.com/auth":{"chatgpt_account_id":"account-123"}}"#);
        let id_token = format!("{header}.{payload}.signature");

        manager.update(&mut app, |manager, ctx| {
            manager
                .store_codex_tokens(
                    TokenResponse {
                        id_token: Some(id_token),
                        access_token: "access-token".into(),
                        refresh_token: Some("refresh-token".into()),
                        expires_in: Some(3600),
                    },
                    ctx,
                )
                .unwrap();
        });

        let revocation = manager
            .update(&mut app, take_chatgpt_tokens_for_disconnect)
            .expect("tokens available for revocation");

        assert_eq!(revocation.access_token.as_deref(), Some("access-token"));
        assert_eq!(revocation.refresh_token.as_deref(), Some("refresh-token"));
        manager.read(&app, |manager, _| {
            assert!(manager.codex_tokens().is_none());
        });
    });
}
