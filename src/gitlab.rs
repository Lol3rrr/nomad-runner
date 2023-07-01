use clap::{Args, Parser};
use serde::Serialize;

/// The Information about this Driver
#[derive(Debug, Serialize)]
pub struct DriverInfo {
    /// The Name of the Driver
    name: String,
    /// The Version of the Driver
    version: String,
}

impl DriverInfo {
    /// Gets the static Driver Info for the current Version
    pub fn new() -> Self {
        Self {
            name: "Nomad-Runner".to_string(),
            version: format!("v{}", env!("CARGO_PKG_VERSION")),
        }
    }
}

#[derive(Debug, Serialize, Args)]
pub struct JobInfo {
    #[serde(rename = "JOB_ID")]
    #[arg(env = "JOB_ID", required = false)]
    pub job_id: String,
}

#[derive(Debug, Serialize)]
pub struct JobConfig {
    pub driver: DriverInfo,
    pub job_env: JobInfo,
}

/// The Environment Information for a given Job
#[derive(Debug, Args)]
pub struct CiEnv {
    #[arg(env = "CUSTOM_ENV_CI_COMMIT_SHA")]
    pub commit_sha: String,

    #[arg(env = "CUSTOM_ENV_CI_JOB_ID")]
    pub ci_job_id: String,

    #[arg(env = "CUSTOM_ENV_CI_JOB_IMAGE")]
    pub job_image: String,
}
