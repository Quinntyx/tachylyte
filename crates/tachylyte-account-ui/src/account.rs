//! Native GPUI surface for account sign-in state.

use crate::model::{AccountIntent, AccountModel, AccountStatus};
use gpui::{div, prelude::*, rgb, Context, ElementId, FocusHandle, Render, Window};

/// A small, model-backed account settings surface.
pub struct AccountSurface {
    pub model: AccountModel,
    pub focus_handle: FocusHandle,
}

impl AccountSurface {
    pub fn new(model: AccountModel, cx: &mut Context<Self>) -> Self {
        Self {
            model,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn mount(model: AccountModel, cx: &mut Context<Self>) -> gpui::Entity<Self> {
        cx.new(|cx| Self::new(model, cx))
    }

    pub fn snapshot(&self) -> AccountModel {
        self.model.clone()
    }

    pub fn drain_intents(&mut self) -> Vec<AccountIntent> {
        self.model.drain_intents()
    }
}

fn action_button(
    id: &'static str,
    label: &'static str,
    target: gpui::Entity<AccountSurface>,
    login: bool,
) -> impl IntoElement {
    div()
        .id(ElementId::Name(id.into()))
        .px_3()
        .py_2()
        .bg(rgb(0x7852ee))
        .text_color(rgb(0xffffff))
        .child(label)
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
            target.update(cx, |surface, cx| {
                if login {
                    surface.model.request_login();
                } else {
                    surface.model.request_logout();
                }
                cx.notify();
            });
        })
}

impl Render for AccountSurface {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let target = cx.entity();
        let content = match &self.model.status {
            AccountStatus::SignedOut => div()
                .id("account-signed-out")
                .child("Not signed in")
                .child(action_button(
                    "account-login",
                    "Log in",
                    target.clone(),
                    true,
                )),
            AccountStatus::SigningIn => div().id("account-signing-in").child("Signing in…"),
            AccountStatus::SignedIn { account_id } => div()
                .id("account-signed-in")
                .child(format!("Signed in as {account_id}"))
                .child(action_button(
                    "account-logout",
                    "Log out",
                    target.clone(),
                    false,
                )),
            AccountStatus::Error(message) => div()
                .id("account-error")
                .child(format!("Account error: {message}"))
                .child(action_button("account-login-error", "Log in", target, true)),
            AccountStatus::Offline => div()
                .id("account-offline")
                .child("Account services are offline"),
        };

        div()
            .id("account-surface")
            .size_full()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x222222))
            .flex()
            .flex_col()
            .p_6()
            .child(div().text_xl().child("Account"))
            .child(div().mt_4().child(content))
    }
}
