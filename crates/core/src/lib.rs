pub mod helper;
pub mod launch;
pub mod logging;
pub mod preset;
pub mod profile;
pub mod rdp;
pub mod registry;
pub mod session;
pub mod store;

pub use helper::{
    HelperClient, HelperConfig, HelperError, HelperProbe, HelperRequestEnvelope, HelperResolve,
    ResolveCredentials, ResolveResult,
};
pub use launch::{
    CredentialFlow, LaunchContext, LaunchIntent, LaunchOutcome, LaunchPlan, LaunchPlanner,
    LaunchPolicy,
};
pub use logging::{
    FileLogger, LogLevel, LoggingError, debug, error, info, init_global_logger, warn,
};
pub use preset::{Preset, PresetDraft, PresetId, PresetSummary};
pub use profile::{
    GatewayUsageMode, Profile, ProfileDraft, ProfileId, ProfileSummary, PromptBehavior, ScreenMode,
    SecurityMode,
};
pub use rdp::{RdpFile, RdpSerializer, SerializedRdp};
pub use registry::{
    PropertyDefinition, PropertyKey, PropertyRegistry, PropertyScope, PropertyType, PropertyValue,
};
pub use session::{ObservedSession, SessionHistoryEntry, SessionState};
pub use store::{
    AppPaths, NewSessionHistoryEntry, ProfileStore, SessionHistoryFilter, SessionHistoryUpdate,
    SqliteStore, StoreError,
};
