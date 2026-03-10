use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use rdp_launch_core::helper::{
    HelperClient, HelperConfig, HelperLaunchContext, HelperProfileRef, HelperTargetRef,
    ResolvePayload,
};
use rdp_launch_core::{
    GatewayUsageMode, LaunchContext, LaunchIntent, LaunchPlanner, LaunchPolicy, PresetDraft,
    ProfileDraft, ProfileStore, PromptBehavior, PropertyRegistry, RdpSerializer, ScreenMode,
    SecurityMode, SqliteStore, info, init_global_logger,
};
use rdp_launch_windows::{
    CmdKeyCredentialBridge, LaunchRuntime, LaunchRuntimeRequest, ProcessSessionTracker,
    SessionTracker, TemporaryCredential, default_app_paths,
};
use thiserror::Error;

#[derive(Parser, Debug)]
#[command(
    name = "rdp-launch",
    about = "Windows MSTSC launcher and local session monitor"
)]
struct Cli {
    #[arg(long)]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Profiles {
        #[command(subcommand)]
        command: ProfilesCommand,
    },
    Launch(LaunchCommand),
    Presets {
        #[command(subcommand)]
        command: PresetsCommand,
    },
    Sessions {
        #[command(subcommand)]
        command: SessionsCommand,
    },
    Helper {
        #[command(subcommand)]
        command: HelperCommand,
    },
}

#[derive(Subcommand, Debug)]
enum ProfilesCommand {
    List,
    Create(ProfileCreateArgs),
    Show(ProfileShowArgs),
}

#[derive(Args, Debug)]
struct ProfileCreateArgs {
    #[arg(long)]
    name: String,
    #[arg(long)]
    full_address: String,
    #[arg(long)]
    username: Option<String>,
    #[arg(long, value_enum, default_value_t = ScreenModeArg::Windowed)]
    screen_mode: ScreenModeArg,
    #[arg(long, default_value_t = false)]
    use_multimon: bool,
    #[arg(long)]
    selected_monitors: Option<String>,
    #[arg(long, default_value_t = true)]
    redirect_clipboard: bool,
    #[arg(long)]
    gateway_hostname: Option<String>,
    #[arg(long, value_enum, default_value_t = GatewayUsageArg::Never)]
    gateway_usage: GatewayUsageArg,
    #[arg(long, value_enum, default_value_t = PromptBehaviorArg::Prompt)]
    prompt_behavior: PromptBehaviorArg,
    #[arg(long, default_value_t = false)]
    allow_windows_credential_bridge: bool,
    #[arg(long, value_enum, default_value_t = SecurityModeArg::Default)]
    security_mode: SecurityModeArg,
}

#[derive(Args, Debug)]
struct ProfileShowArgs {
    profile_id: String,
}

#[derive(Args, Debug)]
struct LaunchCommand {
    profile_id: String,
    #[arg(long)]
    preset_id: Option<String>,
    #[arg(long)]
    helper: Option<String>,
    #[arg(long = "helper-arg")]
    helper_args: Vec<String>,
}

#[derive(Subcommand, Debug)]
enum PresetsCommand {
    List(PresetListArgs),
    Create(PresetCreateArgs),
}

#[derive(Args, Debug)]
struct PresetListArgs {
    profile_id: String,
}

#[derive(Args, Debug)]
struct PresetCreateArgs {
    #[arg(long)]
    profile_id: String,
    #[arg(long)]
    name: String,
    #[arg(long)]
    screen_mode: Option<ScreenModeArg>,
    #[arg(long)]
    use_multimon: Option<bool>,
    #[arg(long)]
    selected_monitors: Option<String>,
    #[arg(long)]
    redirect_clipboard: Option<bool>,
    #[arg(long)]
    gateway_hostname: Option<String>,
    #[arg(long)]
    gateway_usage: Option<GatewayUsageArg>,
    #[arg(long)]
    security_mode: Option<SecurityModeArg>,
}

#[derive(Subcommand, Debug)]
enum SessionsCommand {
    List,
}

#[derive(Subcommand, Debug)]
enum HelperCommand {
    Probe(HelperProbeArgs),
}

#[derive(Args, Debug)]
struct HelperProbeArgs {
    #[arg(long)]
    helper: String,
    #[arg(long = "helper-arg")]
    helper_args: Vec<String>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ScreenModeArg {
    Windowed,
    Fullscreen,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum GatewayUsageArg {
    Never,
    Always,
    Detect,
    Default,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum PromptBehaviorArg {
    Prompt,
    Helper,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum SecurityModeArg {
    Default,
    RemoteGuard,
    RestrictedAdmin,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), CliError> {
    let cli = Cli::parse();
    let command = command_name(&cli.command);
    let app_paths = cli
        .data_dir
        .map(rdp_launch_core::AppPaths::from_root)
        .unwrap_or_else(default_app_paths);
    let _ = init_global_logger(&app_paths, "cli");
    info(
        "cli.startup",
        "cli command starting",
        serde_json::json!({
            "command": command,
            "database": app_paths.database.display().to_string(),
            "log_path": app_paths.app_log.display().to_string(),
        }),
    );
    let store = SqliteStore::open(&app_paths)?;
    let runtime = LaunchRuntime::new(CmdKeyCredentialBridge);
    let _ = runtime.sweep_stale_credentials(&app_paths.root);

    let result = match cli.command {
        Commands::Profiles { command } => run_profiles(&store, command),
        Commands::Launch(command) => run_launch(&store, &app_paths.root, &runtime, command),
        Commands::Presets { command } => run_presets(&store, command),
        Commands::Sessions { command } => run_sessions(&store, command),
        Commands::Helper { command } => run_helper(command),
    };

    if let Err(command_error) = &result {
        rdp_launch_core::error(
            "cli.command.failed",
            "cli command failed",
            serde_json::json!({
                "command": command,
                "error": command_error.to_string(),
            }),
        );
    } else {
        info(
            "cli.command.succeeded",
            "cli command completed",
            serde_json::json!({
                "command": command,
            }),
        );
    }

    result
}

fn run_profiles(store: &SqliteStore, command: ProfilesCommand) -> Result<(), CliError> {
    match command {
        ProfilesCommand::List => {
            for profile in store.list_profiles()? {
                println!(
                    "{}\t{}\t{}\t{}\t{:?}",
                    profile.id,
                    profile.name,
                    profile.full_address,
                    profile.username.as_deref().unwrap_or(""),
                    profile.security_mode
                );
            }
        }
        ProfilesCommand::Create(args) => {
            let profile = store.save_profile(args.into_draft())?;
            info(
                "profiles.create.succeeded",
                "created profile from cli",
                serde_json::json!({
                    "profile_id": &profile.id,
                    "name": &profile.name,
                }),
            );
            println!("{}", profile.id);
        }
        ProfilesCommand::Show(args) => {
            let profile = store
                .get_profile(&args.profile_id)?
                .ok_or_else(|| CliError::NotFound(args.profile_id.clone()))?;
            println!("{}", serde_json::to_string_pretty(&profile)?);
        }
    }

    Ok(())
}

fn run_presets(store: &SqliteStore, command: PresetsCommand) -> Result<(), CliError> {
    match command {
        PresetsCommand::List(args) => {
            for preset in store.list_presets(&args.profile_id)? {
                println!("{}\t{}\t{}", preset.id, preset.profile_id, preset.name);
            }
        }
        PresetsCommand::Create(args) => {
            let preset = store.save_preset(args.into_draft())?;
            info(
                "presets.create.succeeded",
                "created preset from cli",
                serde_json::json!({
                    "preset_id": &preset.id,
                    "profile_id": &preset.profile_id,
                    "name": &preset.name,
                }),
            );
            println!("{}", preset.id);
        }
    }

    Ok(())
}

fn run_launch(
    store: &SqliteStore,
    app_root: &PathBuf,
    runtime: &LaunchRuntime<CmdKeyCredentialBridge>,
    command: LaunchCommand,
) -> Result<(), CliError> {
    info(
        "launch.requested",
        "cli launch requested",
        serde_json::json!({
            "profile_id": &command.profile_id,
            "preset_id": command.preset_id.as_deref(),
            "helper_configured": command.helper.is_some(),
        }),
    );
    let profile = store
        .get_profile(&command.profile_id)?
        .ok_or_else(|| CliError::NotFound(command.profile_id.clone()))?;
    let preset = match &command.preset_id {
        Some(preset_id) => Some(
            store
                .get_preset(preset_id)?
                .ok_or_else(|| CliError::NotFound(preset_id.clone()))?,
        ),
        None => None,
    };

    let helper_result =
        match (&command.helper, profile.prompt_behavior) {
            (Some(helper), PromptBehavior::Helper) => {
                let client = HelperClient::new(HelperConfig {
                    executable: helper.clone(),
                    args: command.helper_args.clone(),
                });
                Some(client.resolve(ResolvePayload {
                    profile: HelperProfileRef {
                        id: profile.id.clone(),
                        name: profile.name.clone(),
                    },
                    target: HelperTargetRef {
                        host: profile.full_address.clone(),
                        port: 3389,
                    },
                    preset: preset.as_ref().map(|preset| {
                        rdp_launch_core::helper::HelperPresetRef {
                            id: preset.id.clone(),
                            name: preset.name.clone(),
                        }
                    }),
                    requested_fields: vec![
                        "username".to_owned(),
                        "password".to_owned(),
                        "domain".to_owned(),
                    ],
                    launch_context: HelperLaunchContext {
                        surface: "cli".to_owned(),
                        reason: "user_launch".to_owned(),
                        allow_windows_vault_bridge: profile.allow_windows_credential_bridge,
                    },
                })?)
            }
            _ => None,
        };

    let planner = LaunchPlanner::new(PropertyRegistry::new());
    let outcome = planner.plan(
        LaunchIntent {
            profile: profile.clone(),
            preset,
            policy: LaunchPolicy {
                allow_prompt: true,
                allow_helper: command.helper.is_some(),
                allow_windows_credential_bridge: profile.allow_windows_credential_bridge,
            },
            context: LaunchContext {
                surface: "cli".to_owned(),
                reason: "user_launch".to_owned(),
            },
        },
        helper_result.clone(),
    )?;

    let serialized_rdp = RdpSerializer::new(PropertyRegistry::new()).serialize(&outcome.plan)?;
    let temporary_credential = helper_result
        .as_ref()
        .and_then(|result| result.credentials.as_ref())
        .and_then(|credentials| {
            Some(TemporaryCredential {
                target: profile.full_address.clone(),
                username: credentials.username.clone()?,
                password: credentials.password.clone()?,
            })
        });

    let artifacts = runtime.launch(
        store,
        app_root,
        LaunchRuntimeRequest {
            profile_id: profile.id,
            profile_name: profile.name,
            target: profile.full_address.clone(),
            serialized_rdp,
            temporary_credential,
        },
    )?;

    println!(
        "{}\t{}\t{}",
        artifacts.history.launch_id,
        artifacts.process.process_id,
        artifacts.rdp_path.display()
    );
    info(
        "launch.completed",
        "cli launch completed",
        serde_json::json!({
            "launch_id": artifacts.history.launch_id,
            "process_id": artifacts.process.process_id,
            "rdp_path": artifacts.rdp_path.display().to_string(),
        }),
    );
    Ok(())
}

fn run_sessions(store: &SqliteStore, command: SessionsCommand) -> Result<(), CliError> {
    match command {
        SessionsCommand::List => {
            for session in ProcessSessionTracker.active_sessions(store)? {
                println!(
                    "{}\t{}\t{}\t{}",
                    session.launch_id, session.profile_name, session.target, session.process_id
                );
            }
        }
    }
    Ok(())
}

fn run_helper(command: HelperCommand) -> Result<(), CliError> {
    match command {
        HelperCommand::Probe(args) => {
            let client = HelperClient::new(HelperConfig {
                executable: args.helper,
                args: args.helper_args,
            });
            let result = client.probe()?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }

    Ok(())
}

fn command_name(command: &Commands) -> &'static str {
    match command {
        Commands::Profiles { .. } => "profiles",
        Commands::Launch(_) => "launch",
        Commands::Presets { .. } => "presets",
        Commands::Sessions { .. } => "sessions",
        Commands::Helper { .. } => "helper",
    }
}

impl ProfileCreateArgs {
    fn into_draft(self) -> ProfileDraft {
        ProfileDraft {
            name: self.name,
            full_address: self.full_address,
            username: self.username,
            screen_mode: match self.screen_mode {
                ScreenModeArg::Windowed => ScreenMode::Windowed,
                ScreenModeArg::Fullscreen => ScreenMode::Fullscreen,
            },
            use_multimon: self.use_multimon,
            selected_monitors: self.selected_monitors,
            redirect_clipboard: self.redirect_clipboard,
            gateway_hostname: self.gateway_hostname,
            gateway_usage: match self.gateway_usage {
                GatewayUsageArg::Never => GatewayUsageMode::Never,
                GatewayUsageArg::Always => GatewayUsageMode::Always,
                GatewayUsageArg::Detect => GatewayUsageMode::Detect,
                GatewayUsageArg::Default => GatewayUsageMode::Default,
            },
            prompt_behavior: match self.prompt_behavior {
                PromptBehaviorArg::Prompt => PromptBehavior::Prompt,
                PromptBehaviorArg::Helper => PromptBehavior::Helper,
            },
            allow_windows_credential_bridge: self.allow_windows_credential_bridge,
            security_mode: match self.security_mode {
                SecurityModeArg::Default => SecurityMode::Default,
                SecurityModeArg::RemoteGuard => SecurityMode::RemoteGuard,
                SecurityModeArg::RestrictedAdmin => SecurityMode::RestrictedAdmin,
            },
        }
    }
}

impl PresetCreateArgs {
    fn into_draft(self) -> PresetDraft {
        PresetDraft {
            profile_id: self.profile_id,
            name: self.name,
            screen_mode: self.screen_mode.map(|value| match value {
                ScreenModeArg::Windowed => ScreenMode::Windowed,
                ScreenModeArg::Fullscreen => ScreenMode::Fullscreen,
            }),
            use_multimon: self.use_multimon,
            selected_monitors: self.selected_monitors,
            redirect_clipboard: self.redirect_clipboard,
            gateway_hostname: self.gateway_hostname,
            gateway_usage: self.gateway_usage.map(|value| match value {
                GatewayUsageArg::Never => GatewayUsageMode::Never,
                GatewayUsageArg::Always => GatewayUsageMode::Always,
                GatewayUsageArg::Detect => GatewayUsageMode::Detect,
                GatewayUsageArg::Default => GatewayUsageMode::Default,
            }),
            security_mode: self.security_mode.map(|value| match value {
                SecurityModeArg::Default => SecurityMode::Default,
                SecurityModeArg::RemoteGuard => SecurityMode::RemoteGuard,
                SecurityModeArg::RestrictedAdmin => SecurityMode::RestrictedAdmin,
            }),
        }
    }
}

#[derive(Debug, Error)]
enum CliError {
    #[error("{0}")]
    CoreStore(#[from] rdp_launch_core::store::StoreError),
    #[error("{0}")]
    Helper(#[from] rdp_launch_core::helper::HelperError),
    #[error("{0}")]
    LaunchPlan(#[from] rdp_launch_core::launch::LaunchPlanError),
    #[error("{0}")]
    Serialize(#[from] rdp_launch_core::rdp::RdpSerializeError),
    #[error("{0}")]
    Runtime(#[from] rdp_launch_windows::launcher::LaunchRuntimeError),
    #[error("{0}")]
    SessionTracker(#[from] rdp_launch_windows::sessions::SessionTrackerError),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("profile `{0}` was not found")]
    NotFound(String),
}
