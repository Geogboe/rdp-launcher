use dioxus::prelude::*;
use rdp_launch_core::{
    GatewayUsageMode, LaunchContext, LaunchIntent, LaunchPlanner, LaunchPolicy, ProfileStore,
    PromptBehavior, PropertyRegistry, RdpSerializer, ScreenMode, SecurityMode, SqliteStore, error,
    info, init_global_logger,
};
use rdp_launch_windows::{
    CmdKeyCredentialBridge, LaunchRuntime, LaunchRuntimeRequest, ProcessSessionTracker,
    default_app_paths, reveal_process_window,
};

use crate::model::{ComposeSurface, HomeViewModel, ProfileForm, Selection};
use crate::windowing::desktop_launch_config;

#[derive(Debug, Clone, PartialEq)]
enum ContextMenuTarget {
    Profile(usize),
    Session(usize),
}

#[derive(Debug, Clone, PartialEq)]
struct ContextMenuState {
    x: f64,
    y: f64,
    target: ContextMenuTarget,
}

#[derive(Debug, Clone)]
enum SelectionIdentity {
    None,
    Profile(String),
    Session {
        launch_id: String,
        profile_id: String,
    },
}

pub fn run() {
    let app_paths = default_app_paths();
    let _ = init_global_logger(&app_paths, "desktop");
    info(
        "desktop.startup",
        "desktop application starting",
        serde_json::json!({
            "database": app_paths.database.display().to_string(),
            "log_path": app_paths.app_log.display().to_string(),
        }),
    );
    LaunchBuilder::desktop()
        .with_cfg(desktop_launch_config())
        .launch(app);
}

fn app() -> Element {
    let mut state = use_signal(load_initial_state);
    let mut context_menu = use_signal(|| None::<ContextMenuState>);
    let current = state.read().clone();
    let search = current.search.clone();
    let selection = current.selection;
    let compose = current.compose;
    let active_context_menu = context_menu.read().clone();
    let session_count = current.sessions.len();
    let profile_count = current.profiles.len();
    let filtered_sessions = current
        .filtered_sessions()
        .into_iter()
        .map(|(index, session)| (index, session.clone()))
        .collect::<Vec<_>>();
    let filtered_profiles = current
        .filtered_profiles()
        .into_iter()
        .map(|(index, profile)| (index, profile.clone()))
        .collect::<Vec<_>>();
    let has_rows = !filtered_sessions.is_empty() || !filtered_profiles.is_empty();

    rsx! {
        style { {APP_CSS} }
        div { class: "app-shell",
            div { class: "window",
                div { class: "topbar",
                    div { class: "brand",
                        div { class: "app-icon" }
                        div {
                            div { class: "brand-title", "RDP Launch" }
                            div { class: "brand-subtitle", "Launch and inspect local MSTSC sessions" }
                        }
                    }
                }

                div { class: "toolbar",
                    input {
                        class: "search-box",
                        value: "{search}",
                        placeholder: "Search connections...",
                        oninput: move |event| state.write().search = event.value()
                    }
                    div { class: "toolbar-actions",
                        button {
                            class: "btn",
                            onclick: move |_| {
                                info(
                                    "desktop.ui.refresh_clicked",
                                    "user refreshed desktop home state",
                                    serde_json::json!({}),
                                );
                                context_menu.set(None);
                                let _ = refresh_home_state(&mut state.write());
                            },
                            "Refresh"
                        }
                        button {
                            class: "toolbar-btn",
                            onclick: move |_| {
                                info(
                                    "desktop.ui.new_profile_clicked",
                                    "user opened the new profile composer",
                                    serde_json::json!({}),
                                );
                                context_menu.set(None);
                                state.write().begin_new_profile();
                            },
                            "+ New"
                        }
                    }
                }

                div { class: "workspace",
                    div { class: "split",
                        div { class: "list-pane",
                            if has_rows {
                                if !filtered_sessions.is_empty() {
                                    div { class: "section-label", "Active Sessions" }
                                    for (index, session) in filtered_sessions {
                                        button {
                                            class: row_class(matches!(selection, Selection::Session(selected) if selected == index), true),
                                            onclick: move |_| {
                                                context_menu.set(None);
                                                info(
                                                    "desktop.ui.session_selected",
                                                    "user selected an active session",
                                                    serde_json::json!({
                                                        "session_index": index,
                                                        "profile_name": &session.profile_name,
                                                        "process_id": session.process_id,
                                                    }),
                                                );
                                                let _ = with_store(|store| {
                                                    state
                                                        .write()
                                                        .select_session(store, index)
                                                        .map_err(|error| error.to_string())
                                                });
                                            },
                                            oncontextmenu: move |event| {
                                                event.prevent_default();
                                                event.stop_propagation();
                                                let point = event.data.client_coordinates();
                                                let _ = with_store(|store| {
                                                    state
                                                        .write()
                                                        .select_session(store, index)
                                                        .map_err(|error| error.to_string())
                                                });
                                                context_menu.set(Some(ContextMenuState {
                                                    x: point.x,
                                                    y: point.y,
                                                    target: ContextMenuTarget::Session(index),
                                                }));
                                            },
                                            div { class: "row-indicator" }
                                            div { class: "row-body",
                                                div { class: "row-title", "{session.profile_name}" }
                                                div { class: "row-host", "{session.target}" }
                                            }
                                            div { class: "row-right",
                                                div { class: "status-active", "Connected" }
                                                div { class: "row-time", "{session.process_id}" }
                                            }
                                        }
                                    }
                                }

                                if !filtered_profiles.is_empty() {
                                    div { class: "section-label", "Connections" }
                                    for (index, profile) in filtered_profiles {
                                        button {
                                            class: row_class(matches!(selection, Selection::Profile(selected) if selected == index), false),
                                            onclick: {
                                                let profile = profile.clone();
                                                move |_| {
                                                    context_menu.set(None);
                                                    info(
                                                        "desktop.ui.profile_selected",
                                                        "user selected a profile",
                                                        serde_json::json!({
                                                            "profile_index": index,
                                                            "profile_id": &profile.id,
                                                            "name": &profile.name,
                                                        }),
                                                    );
                                                    let _ = with_store(|store| {
                                                        state
                                                            .write()
                                                            .select_profile(store, index)
                                                            .map_err(|error| error.to_string())
                                                    });
                                                }
                                            },
                                            ondoubleclick: {
                                                let profile = profile.clone();
                                                move |_| {
                                                    context_menu.set(None);
                                                    info(
                                                        "desktop.ui.profile_double_clicked",
                                                        "user launched a profile from a row double click",
                                                        serde_json::json!({
                                                            "profile_index": index,
                                                            "profile_id": &profile.id,
                                                        }),
                                                    );
                                                    let _ = launch_selected_profile(index, &mut state.write());
                                                }
                                            },
                                            oncontextmenu: move |event| {
                                                event.prevent_default();
                                                event.stop_propagation();
                                                let point = event.data.client_coordinates();
                                                let _ = with_store(|store| {
                                                    state
                                                        .write()
                                                        .select_profile(store, index)
                                                        .map_err(|error| error.to_string())
                                                });
                                                context_menu.set(Some(ContextMenuState {
                                                    x: point.x,
                                                    y: point.y,
                                                    target: ContextMenuTarget::Profile(index),
                                                }));
                                            },
                                            div { class: "row-indicator" }
                                            div { class: "row-body",
                                                div { class: "row-title", "{profile.name}" }
                                                div { class: "row-host", "{profile.full_address}" }
                                            }
                                            div { class: "row-right",
                                                div { class: if matches!(profile.security_mode, SecurityMode::Default) { "status-ready" } else { "status-warn" },
                                                    {security_label(profile.security_mode)}
                                                }
                                                if let Some(username) = profile.username.as_deref() {
                                                    div { class: "row-time", "{display_identity_username(username, &profile.full_address)}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            } else {
                                {render_empty_state(state)}
                            }
                        }

                        aside { class: "inspector",
                            {render_inspector(state)}
                        }
                    }
                }

                div { class: "statusbar",
                    div { class: "statusbar-left",
                        span { class: "status-item", "{session_count} active sessions" }
                        span { class: "status-item", "{profile_count} connections" }
                    }
                    span { class: "status-item", "Store: local SQLite" }
                }
            }

            if !matches!(compose, ComposeSurface::Closed) {
                div { class: "compose-layer",
                    button {
                        class: "compose-backdrop",
                        onclick: move |_| {
                            info(
                                "desktop.ui.compose_cancelled",
                                "user dismissed the compose surface",
                                serde_json::json!({}),
                            );
                            state.write().close_compose();
                        }
                    }
                    aside { class: "compose-panel",
                        {render_compose_surface(state)}
                    }
                }
            }

            if let Some(menu) = active_context_menu {
                div { class: "menu-layer",
                    button {
                        class: "menu-backdrop",
                        onclick: move |_| context_menu.set(None)
                    }
                    {render_context_menu(menu, state, context_menu)}
                }
            }
        }
    }
}

fn render_empty_state(mut state: Signal<HomeViewModel>) -> Element {
    rsx! {
        div { class: "empty-state",
            div { class: "empty-kicker", "No connections yet" }
            div { class: "empty-title", "Create your first connection" }
            p { class: "empty-copy",
                "Saved connections appear here. The inspector stays focused on selected items, while create and edit open in a separate compose panel."
            }
            button {
                class: "btn primary",
                onclick: move |_| {
                    info(
                        "desktop.ui.empty_create_clicked",
                        "user opened new profile composer from empty state",
                        serde_json::json!({}),
                    );
                    state.write().begin_new_profile();
                },
                "Create connection"
            }
        }
    }
}

fn render_inspector(mut state: Signal<HomeViewModel>) -> Element {
    let current = state.read().clone();

    match current.selection {
        Selection::Session(index) => {
            let session = current.sessions[index].clone();
            let preset_name = current.presets.first().map(|preset| preset.name.clone());
            let profile_index = index_from_profile_id(&current, &session.profile_id);
            let session_profile_id_for_reveal = session.profile_id.clone();
            let session_profile_id_for_open = session.profile_id.clone();
            let session_profile_id_for_launch = session.profile_id.clone();

            rsx! {
                div { class: "inspector-header",
                    div { class: "inspector-eyebrow", "Active session" }
                    div { class: "inspector-title", "{session.profile_name}" }
                    div { class: "inspector-host", "{session.target}" }
                }

                div { class: "inspector-section",
                    div { class: "inspector-section-title", "Session" }
                    {detail_row("State", "Connected")}
                    {detail_row_value("PID", session.process_id.to_string())}
                    if let Some(preset_name) = preset_name {
                        {detail_row_value("Preset", preset_name)}
                    }
                }

                div { class: "inspector-actions",
                    button {
                        class: "btn",
                        onclick: move |_| {
                            info(
                                "desktop.ui.reveal_window_clicked",
                                "user requested reveal window",
                                serde_json::json!({
                                    "process_id": session.process_id,
                                    "profile_id": &session_profile_id_for_reveal,
                                }),
                            );
                            let _ = reveal_process_window(session.process_id);
                        },
                        "Reveal window"
                    }
                    button {
                        class: "btn",
                        onclick: move |_| {
                            if let Some(profile_index) = profile_index {
                                info(
                                    "desktop.ui.open_profile_clicked",
                                    "user opened a profile from session inspector",
                                    serde_json::json!({
                                        "profile_id": &session_profile_id_for_open,
                                        "profile_index": profile_index,
                                    }),
                                );
                                let _ = with_store(|store| {
                                    state
                                        .write()
                                        .select_profile(store, profile_index)
                                        .map_err(|error| error.to_string())
                                });
                            }
                        },
                        "Open profile"
                    }
                    button {
                        class: "btn primary",
                        onclick: move |_| {
                            if let Some(profile_index) = profile_index {
                                info(
                                    "desktop.ui.launch_from_session_clicked",
                                    "user launched a new session from active-session inspector",
                                    serde_json::json!({
                                        "profile_id": &session_profile_id_for_launch,
                                        "profile_index": profile_index,
                                    }),
                                );
                                let _ = launch_selected_profile(profile_index, &mut state.write());
                            }
                        },
                        "Launch new"
                    }
                }
            }
        }
        Selection::Profile(index) => {
            let profile = current.profiles[index].clone();
            let presets = current.presets.clone();
            let selected_preset_id = current.selected_preset_id.clone();
            let profile_id_for_preset = profile.id.clone();
            let profile_id_for_edit = profile.id.clone();
            let profile_id_for_launch = profile.id.clone();
            let profile_id_for_delete = profile.id.clone();
            let (identity_username, identity_domain) =
                split_identity_username(profile.username.as_deref(), &profile.full_address);

            rsx! {
                div { class: "inspector-header",
                    div { class: "inspector-eyebrow", "Connection" }
                    div { class: "inspector-title", "{profile.name}" }
                    div { class: "inspector-host", "{profile.full_address}" }
                }

                div { class: "inspector-section",
                    div { class: "inspector-section-title", "Identity" }
                    {detail_row_value("Display name", profile.name.clone())}
                    {detail_row_value("Hostname", profile.full_address.clone())}
                    if !identity_username.is_empty() {
                        {detail_row_value("Username", identity_username)}
                    }
                    if !identity_domain.is_empty() {
                        {detail_row_value("Domain", identity_domain)}
                    }
                }

                div { class: "inspector-section",
                    div { class: "inspector-section-title", "Connection" }
                    {detail_row("Security", match profile.security_mode {
                        SecurityMode::Default => "Default",
                        SecurityMode::RemoteGuard => "Remote Guard",
                        SecurityMode::RestrictedAdmin => "Restricted Admin",
                    })}
                }

                div { class: "inspector-section",
                    div { class: "inspector-section-title", "Status" }
                    {detail_row("Ready", "Yes")}
                    {detail_row("Last launch", if profile.last_used_at.is_some() { "Recorded" } else { "Not yet launched" })}
                }

                if !presets.is_empty() {
                    div { class: "inspector-section",
                        div { class: "inspector-section-title", "Preset" }
                        select {
                            class: "field",
                            onchange: move |event| {
                                let value = event.value();
                                info(
                                    "desktop.ui.preset_changed",
                                    "user changed selected preset",
                                    serde_json::json!({
                                        "profile_id": &profile_id_for_preset,
                                        "preset_id": if value.is_empty() { None::<&str> } else { Some(value.as_str()) },
                                    }),
                                );
                                state.write().set_selected_preset(if value.is_empty() { None } else { Some(value) });
                            },
                            option { value: "", "Base profile" }
                            for preset in presets.iter() {
                                option {
                                    value: "{preset.id}",
                                    selected: selected_preset_id.as_deref() == Some(preset.id.as_str()),
                                    "{preset.name}"
                                }
                            }
                        }
                    }
                }

                div { class: "inspector-actions",
                    button {
                        class: "btn",
                        onclick: move |_| {
                            info(
                                "desktop.ui.edit_profile_clicked",
                                "user opened the edit profile composer",
                                serde_json::json!({
                                    "profile_id": &profile_id_for_edit,
                                    "profile_index": index,
                                }),
                            );
                            let _ = with_store(|store| {
                                state
                                    .write()
                                    .begin_edit_profile(store, index)
                                    .map_err(|error| error.to_string())
                            });
                        },
                        "Edit"
                    }
                    button {
                        class: "btn danger",
                        onclick: move |_| {
                            info(
                                "desktop.ui.delete_profile_clicked",
                                "user deleted a profile from inspector",
                                serde_json::json!({
                                    "profile_id": &profile_id_for_delete,
                                    "profile_index": index,
                                }),
                            );
                            let _ = delete_profile(index, &mut state.write());
                        },
                        "Delete"
                    }
                    button {
                        class: "btn primary",
                        onclick: move |_| {
                            info(
                                "desktop.ui.launch_clicked",
                                "user launched a profile from inspector",
                                serde_json::json!({
                                    "profile_id": &profile_id_for_launch,
                                    "profile_index": index,
                                }),
                            );
                            let _ = launch_selected_profile(index, &mut state.write());
                        },
                        "Launch"
                    }
                }
            }
        }
        Selection::None => rsx! {
            div { class: "inspector-empty",
                div { class: "inspector-eyebrow", "Inspector" }
                div { class: "inspector-title", "Select a connection" }
                p { class: "empty-copy",
                    "Choose a saved connection or active session to inspect it here. New and edit happen in the separate compose panel."
                }
            }
        },
    }
}

fn render_compose_surface(mut state: Signal<HomeViewModel>) -> Element {
    let current = state.read().clone();
    let form = current.profile_form.clone();
    let compose = current.compose;
    let heading = match compose {
        ComposeSurface::New => "New connection",
        ComposeSurface::Edit(_) => "Edit connection",
        ComposeSurface::Closed => "Connection",
    };
    let subheading = match compose {
        ComposeSurface::New => {
            "Create a saved MSTSC profile with the focused field set from slice one."
        }
        ComposeSurface::Edit(_) => {
            "Update the focused connection fields without losing list context."
        }
        ComposeSurface::Closed => "",
    };

    rsx! {
        div { class: "compose-header",
            div {
                div { class: "compose-eyebrow", "Compose" }
                div { class: "compose-title", "{heading}" }
                p { class: "compose-copy", "{subheading}" }
            }
            button {
                class: "icon-btn",
                onclick: move |_| {
                    info(
                        "desktop.ui.compose_cancelled",
                        "user cancelled the compose surface",
                        serde_json::json!({}),
                    );
                    state.write().close_compose();
                },
                "Close"
            }
        }

        form {
            class: "compose-form",
            onsubmit: move |_| {
                let _ = save_profile(&mut state.write());
            },
            div { class: "compose-section",
                div { class: "compose-section-title", "Identity" }
                input {
                    class: "field",
                    value: "{form.display_name}",
                    placeholder: "Display name",
                    oninput: move |event| state.write().profile_form.display_name = event.value()
                }
                input {
                    class: "field",
                    value: "{form.hostname}",
                    placeholder: "Hostname or address",
                    oninput: move |event| {
                        let next_hostname = event.value();
                        let mut model = state.write();
                        let old_hostname = model.profile_form.hostname.clone();
                        model.profile_form.hostname = next_hostname;
                        if model.profile_form.domain == old_hostname {
                            model.profile_form.domain = model.profile_form.hostname.clone();
                        }
                    }
                }
                input {
                    class: "field",
                    value: "{form.username}",
                    placeholder: "Username",
                    oninput: move |event| state.write().profile_form.apply_username_input(&event.value())
                }
                input {
                    class: "field",
                    value: "{form.domain}",
                    placeholder: "Domain or UPN suffix",
                    oninput: move |event| state.write().profile_form.apply_domain_input(&event.value())
                }
                p { class: "compose-copy",
                    "Typing DOMAIN\\user or user@domain.example in Username will split it automatically."
                }
            }

            div { class: "compose-section",
                div { class: "compose-section-title", "Display" }
                div { class: "form-row",
                    label { "Screen mode" }
                    select {
                        class: "field",
                        onchange: move |event| state.write().profile_form.screen_mode = if event.value() == "fullscreen" { ScreenMode::Fullscreen } else { ScreenMode::Windowed },
                        option { value: "windowed", selected: matches!(form.screen_mode, ScreenMode::Windowed), "Windowed" }
                        option { value: "fullscreen", selected: matches!(form.screen_mode, ScreenMode::Fullscreen), "Fullscreen" }
                    }
                }
                label { class: "checkbox",
                    input {
                        r#type: "checkbox",
                        checked: form.use_multimon,
                        oninput: move |event| state.write().profile_form.use_multimon = event.checked()
                    }
                    span { "Use multimon" }
                }
                input {
                    class: "field",
                    value: "{form.selected_monitors}",
                    placeholder: "Selected monitors",
                    oninput: move |event| state.write().profile_form.selected_monitors = event.value()
                }
                label { class: "checkbox",
                    input {
                        r#type: "checkbox",
                        checked: form.redirect_clipboard,
                        oninput: move |event| state.write().profile_form.redirect_clipboard = event.checked()
                    }
                    span { "Redirect clipboard" }
                }
            }

            div { class: "compose-section",
                div { class: "compose-section-title", "Gateway and security" }
                input {
                    class: "field",
                    value: "{form.gateway_hostname}",
                    placeholder: "Gateway hostname",
                    oninput: move |event| state.write().profile_form.gateway_hostname = event.value()
                }
                div { class: "form-row",
                    label { "Gateway usage" }
                    select {
                        class: "field",
                        onchange: move |event| state.write().profile_form.gateway_usage = match event.value().as_str() {
                            "always" => GatewayUsageMode::Always,
                            "detect" => GatewayUsageMode::Detect,
                            "default" => GatewayUsageMode::Default,
                            _ => GatewayUsageMode::Never,
                        },
                        option { value: "never", selected: matches!(form.gateway_usage, GatewayUsageMode::Never), "Never" }
                        option { value: "always", selected: matches!(form.gateway_usage, GatewayUsageMode::Always), "Always" }
                        option { value: "detect", selected: matches!(form.gateway_usage, GatewayUsageMode::Detect), "Detect" }
                        option { value: "default", selected: matches!(form.gateway_usage, GatewayUsageMode::Default), "Default" }
                    }
                }
                div { class: "form-row",
                    label { "Security" }
                    select {
                        class: "field",
                        onchange: move |event| state.write().profile_form.security_mode = match event.value().as_str() {
                            "remote_guard" => SecurityMode::RemoteGuard,
                            "restricted_admin" => SecurityMode::RestrictedAdmin,
                            _ => SecurityMode::Default,
                        },
                        option { value: "default", selected: matches!(form.security_mode, SecurityMode::Default), "Default" }
                        option { value: "remote_guard", selected: matches!(form.security_mode, SecurityMode::RemoteGuard), "Remote Guard" }
                        option { value: "restricted_admin", selected: matches!(form.security_mode, SecurityMode::RestrictedAdmin), "Restricted Admin" }
                    }
                }
            }

            div { class: "compose-actions",
                button {
                    class: "btn",
                    r#type: "button",
                    onclick: move |_| {
                        info(
                            "desktop.ui.compose_cancelled",
                            "user cancelled the compose surface",
                            serde_json::json!({}),
                        );
                        state.write().close_compose();
                    },
                    "Cancel"
                }
                button { class: "btn primary", r#type: "submit", "Save connection" }
            }
        }
    }
}

fn save_profile(state: &mut HomeViewModel) -> Result<(), String> {
    let app_paths = default_app_paths();
    let store = SqliteStore::open(&app_paths).map_err(|error| error.to_string())?;

    let saved_profile_id = match state.compose {
        ComposeSurface::Edit(index) => {
            let profile_id = state
                .profiles
                .get(index)
                .ok_or_else(|| "missing profile".to_owned())?
                .id
                .clone();
            store
                .update_profile(&profile_id, state.profile_form.to_draft())
                .map_err(|error| error.to_string())?;
            info(
                "desktop.profile.updated",
                "updated profile from desktop composer",
                serde_json::json!({
                    "profile_id": &profile_id,
                    "name": &state.profile_form.display_name,
                }),
            );
            profile_id
        }
        ComposeSurface::New => {
            let profile = store
                .save_profile(state.profile_form.to_draft())
                .map_err(|error| error.to_string())?;
            info(
                "desktop.profile.created",
                "created profile from desktop composer",
                serde_json::json!({
                    "profile_id": &profile.id,
                    "name": &profile.name,
                }),
            );
            profile.id
        }
        ComposeSurface::Closed => return Err("compose surface is not open".to_owned()),
    };

    let mut refreshed = reload_state_preserving_view(state, &store)?;
    refreshed
        .select_profile_by_id(&store, &saved_profile_id)
        .map_err(|error| error.to_string())?;
    refreshed.close_compose();
    *state = refreshed;
    Ok(())
}

fn launch_selected_profile(index: usize, state: &mut HomeViewModel) -> Result<(), String> {
    let app_paths = default_app_paths();
    let store = SqliteStore::open(&app_paths).map_err(|error| error.to_string())?;
    let profile_id = state
        .profiles
        .get(index)
        .ok_or_else(|| "missing profile".to_owned())?
        .id
        .clone();
    info(
        "desktop.launch.requested",
        "desktop launch requested",
        serde_json::json!({
            "profile_id": &profile_id,
            "selected_preset_id": state.selected_preset_id.as_deref(),
        }),
    );
    let profile = store
        .get_profile(&profile_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "missing profile".to_owned())?;

    let planner = LaunchPlanner::new(PropertyRegistry::new());
    let mut desktop_profile = profile.clone();
    desktop_profile.prompt_behavior = PromptBehavior::Prompt;
    desktop_profile.allow_windows_credential_bridge = false;
    let outcome = planner
        .plan(
            LaunchIntent {
                profile: desktop_profile,
                preset: state
                    .selected_preset_id
                    .as_ref()
                    .and_then(|preset_id| store.get_preset(preset_id).ok().flatten()),
                policy: LaunchPolicy {
                    allow_prompt: true,
                    allow_helper: false,
                    allow_windows_credential_bridge: false,
                },
                context: LaunchContext {
                    surface: "desktop".to_owned(),
                    reason: "user_launch".to_owned(),
                },
            },
            None,
        )
        .map_err(|error| error.to_string())?;

    let serialized_rdp = RdpSerializer::new(PropertyRegistry::new())
        .serialize(&outcome.plan)
        .map_err(|error| error.to_string())?;

    let runtime = LaunchRuntime::new(CmdKeyCredentialBridge);
    let launched_profile_id = profile.id.clone();
    let launched_target = profile.full_address.clone();
    runtime
        .launch(
            &store,
            &app_paths.root,
            LaunchRuntimeRequest {
                profile_id: profile.id,
                profile_name: profile.name,
                target: profile.full_address,
                serialized_rdp,
                temporary_credential: None,
            },
        )
        .map_err(|error| error.to_string())?;
    info(
        "desktop.launch.completed",
        "desktop launch completed",
        serde_json::json!({
            "profile_id": launched_profile_id,
            "target": launched_target,
        }),
    );

    *state = reload_state_preserving_view(state, &store)?;
    Ok(())
}

fn load_initial_state() -> HomeViewModel {
    let app_paths = default_app_paths();
    let runtime = LaunchRuntime::new(CmdKeyCredentialBridge);
    if let Ok(removed) = runtime.sweep_stale_credentials(&app_paths.root) {
        if !removed.is_empty() {
            info(
                "desktop.startup.swept_bridge_targets",
                "desktop startup swept stale credential bridge targets",
                serde_json::json!({
                    "target_count": removed.len(),
                }),
            );
        }
    }
    match SqliteStore::open(&app_paths).and_then(|store| {
        HomeViewModel::load(&store, &ProcessSessionTracker)
            .map_err(|error| rdp_launch_core::StoreError::MissingProfile(error.to_string()))
    }) {
        Ok(model) => {
            info(
                "desktop.state.loaded",
                "loaded desktop home state",
                serde_json::json!({
                    "profiles": model.profiles.len(),
                    "sessions": model.sessions.len(),
                    "presets": model.presets.len(),
                }),
            );
            model
        }
        Err(error_message) => {
            error(
                "desktop.state.load_failed",
                "failed to load desktop home state; falling back to empty model",
                serde_json::json!({
                    "error": error_message.to_string(),
                }),
            );
            HomeViewModel {
                search: String::new(),
                profiles: Vec::new(),
                sessions: Vec::new(),
                presets: Vec::new(),
                selected_preset_id: None,
                selection: Selection::None,
                compose: ComposeSurface::Closed,
                profile_form: ProfileForm::default(),
            }
        }
    }
}

fn detail_row(label: &'static str, value: &'static str) -> Element {
    rsx! {
        div { class: "detail-row",
            span { class: "detail-label", "{label}" }
            span { class: "detail-value", "{value}" }
        }
    }
}

fn detail_row_value(label: &'static str, value: String) -> Element {
    rsx! {
        div { class: "detail-row",
            span { class: "detail-label", "{label}" }
            span { class: "detail-value", "{value}" }
        }
    }
}

fn row_class(selected: bool, active: bool) -> &'static str {
    match (selected, active) {
        (true, true) => "row selected active",
        (true, false) => "row selected",
        (false, true) => "row active",
        (false, false) => "row",
    }
}

const APP_CSS: &str = include_str!("app.css");

fn with_store<T>(mut f: impl FnMut(&SqliteStore) -> Result<T, String>) -> Result<T, String> {
    let app_paths = default_app_paths();
    let store = SqliteStore::open(&app_paths).map_err(|error| error.to_string())?;
    f(&store)
}

fn index_from_profile_id(state: &HomeViewModel, profile_id: &str) -> Option<usize> {
    state
        .profiles
        .iter()
        .position(|profile| profile.id == profile_id)
}

fn render_context_menu(
    menu: ContextMenuState,
    mut state: Signal<HomeViewModel>,
    mut context_menu: Signal<Option<ContextMenuState>>,
) -> Element {
    let menu_style = format!("left: {}px; top: {}px;", menu.x, menu.y);

    match menu.target {
        ContextMenuTarget::Profile(index) => {
            rsx! {
                div { class: "context-menu", style: "{menu_style}",
                    button {
                        class: "context-menu-item",
                        onclick: move |_| {
                            context_menu.set(None);
                            let _ = launch_selected_profile(index, &mut state.write());
                        },
                        "Launch"
                    }
                    button {
                        class: "context-menu-item",
                        onclick: move |_| {
                            context_menu.set(None);
                            let _ = with_store(|store| {
                                state
                                    .write()
                                    .begin_edit_profile(store, index)
                                    .map_err(|error| error.to_string())
                            });
                        },
                        "Edit"
                    }
                    button {
                        class: "context-menu-item danger",
                        onclick: move |_| {
                            context_menu.set(None);
                            let _ = delete_profile(index, &mut state.write());
                        },
                        "Delete"
                    }
                }
            }
        }
        ContextMenuTarget::Session(index) => {
            rsx! {
                div { class: "context-menu", style: "{menu_style}",
                    button {
                        class: "context-menu-item",
                        onclick: move |_| {
                            context_menu.set(None);
                            if let Some(session) = state.read().sessions.get(index) {
                                let _ = reveal_process_window(session.process_id);
                            }
                        },
                        "Reveal window"
                    }
                    button {
                        class: "context-menu-item",
                        onclick: move |_| {
                            context_menu.set(None);
                            let profile_index = {
                                let current = state.read().clone();
                                current
                                    .sessions
                                    .get(index)
                                    .and_then(|session| index_from_profile_id(&current, &session.profile_id))
                            };
                            if let Some(profile_index) = profile_index {
                                let _ = with_store(|store| {
                                    state
                                        .write()
                                        .select_profile(store, profile_index)
                                        .map_err(|error| error.to_string())
                                });
                            }
                        },
                        "Open profile"
                    }
                    button {
                        class: "context-menu-item",
                        onclick: move |_| {
                            context_menu.set(None);
                            let profile_index = {
                                let current = state.read().clone();
                                current
                                    .sessions
                                    .get(index)
                                    .and_then(|session| index_from_profile_id(&current, &session.profile_id))
                            };
                            if let Some(profile_index) = profile_index {
                                let _ = launch_selected_profile(profile_index, &mut state.write());
                            }
                        },
                        "Launch new"
                    }
                }
            }
        }
    }
}

fn refresh_home_state(state: &mut HomeViewModel) -> Result<(), String> {
    let app_paths = default_app_paths();
    let store = SqliteStore::open(&app_paths).map_err(|error| error.to_string())?;
    *state = reload_state_preserving_view(state, &store)?;
    Ok(())
}

fn delete_profile(index: usize, state: &mut HomeViewModel) -> Result<(), String> {
    let app_paths = default_app_paths();
    let store = SqliteStore::open(&app_paths).map_err(|error| error.to_string())?;
    let profile = state
        .profiles
        .get(index)
        .ok_or_else(|| "missing profile".to_owned())?
        .clone();
    store
        .delete_profile(&profile.id)
        .map_err(|error| error.to_string())?;
    info(
        "desktop.profile.deleted",
        "deleted profile from desktop shell",
        serde_json::json!({
            "profile_id": profile.id,
            "name": profile.name,
        }),
    );
    *state = reload_state_preserving_view(state, &store)?;
    Ok(())
}

fn reload_state_preserving_view(
    state: &HomeViewModel,
    store: &SqliteStore,
) -> Result<HomeViewModel, String> {
    let selection_identity = selection_identity(state);
    let mut refreshed =
        HomeViewModel::load(store, &ProcessSessionTracker).map_err(|error| error.to_string())?;
    refreshed.search = state.search.clone();
    match selection_identity {
        SelectionIdentity::None => {}
        SelectionIdentity::Profile(profile_id) => {
            let _ = refreshed.select_profile_by_id(store, &profile_id);
        }
        SelectionIdentity::Session {
            launch_id,
            profile_id,
        } => {
            if let Some(index) = refreshed
                .sessions
                .iter()
                .position(|session| session.launch_id == launch_id)
            {
                let _ = refreshed.select_session(store, index);
            } else {
                let _ = refreshed.select_profile_by_id(store, &profile_id);
            }
        }
    }
    Ok(refreshed)
}

fn selection_identity(state: &HomeViewModel) -> SelectionIdentity {
    match state.selection {
        Selection::None => SelectionIdentity::None,
        Selection::Profile(index) => state
            .profiles
            .get(index)
            .map(|profile| SelectionIdentity::Profile(profile.id.clone()))
            .unwrap_or(SelectionIdentity::None),
        Selection::Session(index) => state
            .sessions
            .get(index)
            .map(|session| SelectionIdentity::Session {
                launch_id: session.launch_id.clone(),
                profile_id: session.profile_id.clone(),
            })
            .unwrap_or(SelectionIdentity::None),
    }
}

fn security_label(security_mode: SecurityMode) -> &'static str {
    match security_mode {
        SecurityMode::Default => "Standard",
        SecurityMode::RemoteGuard => "Remote Guard",
        SecurityMode::RestrictedAdmin => "Restricted Admin",
    }
}

fn split_identity_username(username: Option<&str>, hostname: &str) -> (String, String) {
    let Some(username) = username.map(str::trim).filter(|value| !value.is_empty()) else {
        return (String::new(), String::new());
    };

    if let Some((user, domain)) = username.split_once('@') {
        return (user.to_owned(), domain.to_owned());
    }

    if let Some((domain, user)) = username.split_once('\\') {
        let domain = if domain == "." { hostname } else { domain };
        return (user.to_owned(), domain.to_owned());
    }

    (username.to_owned(), String::new())
}

fn display_identity_username(username: &str, hostname: &str) -> String {
    let (user, domain) = split_identity_username(Some(username), hostname);
    if domain.is_empty() {
        user
    } else {
        format!("{user} @ {domain}")
    }
}
