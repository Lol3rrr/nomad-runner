#![warn(clippy::unwrap_used)]

use std::{borrow::Cow, path::Path};

use crate::nomad::{events, job};

mod gitlab;
pub mod nomad;

mod exec;
use exec::ExecSession;

pub use gitlab::{CiEnv, JobInfo};

use log::debug;
use rand::Rng;

const JOB_NAME: &str = "Job";
const MANAGEMENT_NAME: &str = "Manage";

/// The Nomad Configuration
///
/// This is needed to configure the correct Access to a Nomad Cluster
#[derive(Debug)]
pub struct NomadConfig {
    pub endpoint: NomadEndpoint,
    pub address: String,
    pub port: u16,
    pub datacenters: Vec<String>,
}

#[derive(Debug)]
pub enum NomadEndpoint {
    HTTP { address: String, port: u16 },
    UnixSocket { path: String, token: String },
}

impl NomadConfig {
    /// Creates the Nomad Configuration with default starting Values and then overwrites these
    /// Values with values loaded from Environment Values
    ///
    /// # Environment Variables
    /// * `NOMAD_ADDR`: The Nomad address
    /// * `NOMAD_PORT`: The Nomad Port
    /// * `NOMAD_DATACENTER`: The Datacenter in which to run the Jobs
    /// * `NOMAD_SOCKET`: The Secret Dir
    /// * `NOMAD_TOKEN`: The nomad token
    pub fn load_with_defaults() -> Self {
        let mut raw = Self {
            endpoint: NomadEndpoint::HTTP {
                address: "127.0.0.1".to_string(),
                port: 4646,
            },
            address: "127.0.0.1".to_string(),
            port: 4646,
            datacenters: vec!["dc1".to_string()],
        };

        let task_api_vars = std::env::var("NOMAD_SOCKET")
            .ok()
            .and_then(|dir| std::env::var("NOMAD_TOKEN").ok().map(|token| (dir, token)));

        let address = std::env::var("NOMAD_ADDR").unwrap_or("127.0.0.1".to_string());
        let port = std::env::var("NOMAD_PORT")
            .ok()
            .map(|rp| rp.parse::<u16>().ok())
            .flatten()
            .unwrap_or(4646);

        raw.address = address.clone();
        raw.port = port;

        raw.endpoint = match task_api_vars {
            Some((dir, token)) => NomadEndpoint::UnixSocket {
                path: dir,
                token,
            },
            None => NomadEndpoint::HTTP { address, port },
        };

        // Search for set environment variables
        for (key, value) in std::env::vars() {
            match key.as_str() {
                "NOMAD_DATACENTER" => {
                    raw.datacenters = vec![value];
                }
                _ => {}
            };
        }

        raw
    }
}

pub fn config(ci_env: &CiEnv) -> gitlab::JobConfig {
    let rid: u64 = rand::thread_rng().gen();

    gitlab::JobConfig {
        driver: gitlab::DriverInfo::new(),
        job_env: gitlab::JobInfo {
            job_id: format!("ci-{}-{:x}", ci_env.ci_job_id, rid),
        },
    }
}

#[derive(Debug, Clone)]
pub enum RunSubStage {
    PrepareScript,
    GetSources,
    RestoreCache,
    DownloadArtifacts,
    Step(String),
    BuildScript,
    AfterScript,
    ArchiveCache,
    ArchiveCacheOnFailure,
    UploadArtifactsOnSuccess,
    UploadArtifactsOnFailure,
    CleanupFileVariables,
}

impl From<String> for RunSubStage {
    fn from(input: String) -> Self {
        for tmp in Self::value_variants() {
            let tmp_str = match tmp.to_possible_value() {
                Some(p) => p,
                None => continue,
            };

            if tmp_str.matches(&input, false) {
                return tmp.clone();
            }
        }

        if input.starts_with("step_script") {
            return Self::BuildScript;
        }

        if input.starts_with("step_") {
            let name = input.strip_prefix("step_").unwrap();

            return Self::Step(name.to_string());
        }

        todo!()
    }
}

impl RunSubStage {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Self::PrepareScript,
            Self::GetSources,
            Self::RestoreCache,
            Self::DownloadArtifacts,
            Self::BuildScript,
            Self::AfterScript,
            Self::ArchiveCache,
            Self::ArchiveCacheOnFailure,
            Self::UploadArtifactsOnSuccess,
            Self::UploadArtifactsOnFailure,
            Self::CleanupFileVariables,
        ]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        match self {
            Self::PrepareScript => Some(clap::builder::PossibleValue::new("prepare_script")),
            Self::GetSources => Some(clap::builder::PossibleValue::new("get_sources")),
            Self::RestoreCache => Some(clap::builder::PossibleValue::new("restore_cache")),
            Self::DownloadArtifacts => {
                Some(clap::builder::PossibleValue::new("download_artifacts"))
            }
            Self::BuildScript => Some(clap::builder::PossibleValue::new("build_script")),
            Self::AfterScript => Some(clap::builder::PossibleValue::new("after_script")),
            Self::ArchiveCache => Some(clap::builder::PossibleValue::new("archive_cache")),
            Self::ArchiveCacheOnFailure => Some(clap::builder::PossibleValue::new(
                "archive_cache_on_failure",
            )),
            Self::UploadArtifactsOnSuccess => Some(clap::builder::PossibleValue::new(
                "upload_artifacts_on_success",
            )),
            Self::UploadArtifactsOnFailure => Some(clap::builder::PossibleValue::new(
                "upload_artifacts_on_failure",
            )),
            Self::CleanupFileVariables => {
                Some(clap::builder::PossibleValue::new("cleanup_file_variables"))
            }
            _ => None,
        }
    }
}

/// Runs the Prepare Stage from the Gitlab Runner
///
/// # Actions
/// * Creates a new Batch Job in Nomad for this Gitlab Job
pub async fn prepare(
    config: &NomadConfig,
    info: &gitlab::JobInfo,
    ci_env: &CiEnv,
) -> Result<(), ()> {
    let job_spec = job::Spec {
        datacenters: config.datacenters.clone(),
        id: info.job_id.clone(),
        ty: job::Ty::Batch,
        task_groups: vec![job::TaskGroup {
            name: "Job".to_string(),
            networks: Vec::new(),
            services: Vec::new(),
            tasks: vec![
                job::Task {
                    name: JOB_NAME.to_string(),
                    config: job::TaskConfig::Docker {
                        image: ci_env.job_image.clone(),
                        entrypoint: vec!["/bin/bash".to_string()],
                        interactive: true,
                        volumes: vec![],
                        work_dir: "/alloc/".to_string(),
                        mounts: vec![],
                    },
                    env: [("GIT_SSL_NO_VERIFY".to_string(), "true".to_string())]
                        .into_iter()
                        .collect(),
                    resources: job::TaskResources {
                        cpu: 5000,
                        memory_mb: 4000,
                    },
                },
                job::Task {
                    name: MANAGEMENT_NAME.to_string(),
                    config: job::TaskConfig::Docker {
                        image: "gitlab/gitlab-runner:latest".to_string(),
                        entrypoint: vec!["/bin/bash".to_string()],
                        interactive: true,
                        volumes: vec![],
                        work_dir: "/alloc".to_string(),
                        mounts: vec![],
                    },
                    env: [("GIT_SSL_NO_VERIFY".to_string(), "true".to_string())]
                        .into_iter()
                        .collect(),
                    resources: job::TaskResources {
                        cpu: 100,
                        memory_mb: 300,
                    },
                },
            ],
        }],
    };

    match &config.endpoint {
        NomadEndpoint::HTTP { address, port } => {
            println!("Using normal HTTP endpoint");
        }
        NomadEndpoint::UnixSocket { path, token } => {
            println!("Using unix domain socket");
        }
    };

    let nomad_client = config_to_client(config);

    println!("Checking for existing Job...");

    let prev_res = nomad_client
        .get_job_allocations(&info.job_id)
        .await
        .map_err(|e| ())?;

    if !prev_res.is_empty()
        && prev_res.into_iter().all(|alloc| {
            alloc
                .task_states
                .into_iter()
                .any(|task| task.1.state.eq_ignore_ascii_case("running"))
        })
    {
        panic!(
            "There already exists a Job with the ID '{}' running",
            info.job_id
        );
    }

    println!("Starting Job as '{}'...", info.job_id);

    let res_body = nomad_client.run_job(job_spec).await.unwrap();
    debug!("Body: {:?}", res_body);

    let mut node_name = String::new();

    let mut event_stream = nomad_client
        .events(res_body.index, Some(&[events::Topic::Allocation]))
        .await
        .map_err(|e| ())?;

    while let Some(tmp) = event_stream.recv().await {
        if tmp.topic != events::Topic::Allocation {
            continue;
        }

        let alloction_data = match tmp.payload.allocation {
            Some(s) => s,
            None => {
                println!("[Warn] Payload was missing from event");
                continue;
            }
        };

        node_name = alloction_data.node_name;

        if alloction_data.job_id != info.job_id {
            continue;
        }

        let running = !alloction_data.task_states.is_empty()
            && alloction_data
                .task_states
                .values()
                .all(|state| state.state.eq_ignore_ascii_case("running"));

        if running {
            break;
        }
    }

    println!("Job has started on {}.", node_name);

    Ok(())
}

#[derive(Debug)]
pub enum RunError {
    LoadingJobAllocations(nomad::ClientRequestError),
    StartingExecSession {
        error: exec::StartError,
        ctx: &'static str,
    },
    Other(Cow<'static, str>),
}

fn config_to_client(config: &NomadConfig) -> nomad::Client {
    match &config.endpoint {
        NomadEndpoint::HTTP { address, port } => {
            nomad::Client::new(config.address.clone(), config.port)
        }
        NomadEndpoint::UnixSocket { path, token } => {
            nomad::Client::new_socket(config.address.clone(), config.port, path.into(), token.clone())
        }
    }
}

/// Runs the Run Stage for the Gitlab Runner
pub async fn run(
    config: &NomadConfig,
    info: &gitlab::JobInfo,
    script_path: &Path,
    sub_stage: RunSubStage,
) -> Result<i32, RunError> {
    let nomad_client = config_to_client(config);

    let content = nomad_client
        .get_job_allocations(&info.job_id)
        .await
        .map_err(|e| RunError::LoadingJobAllocations(e))?;

    let running_alloc = match content.into_iter().find(|alloc| {
        alloc
            .task_states
            .iter()
            .all(|(_, state)| !state.failed && state.state == "running")
    }) {
        Some(a) => a,
        None => {
            println!("Not running job found");
            todo!("No running allocation");
        }
    };

    let job_name = match sub_stage {
        RunSubStage::BuildScript | RunSubStage::AfterScript => JOB_NAME,
        _ => MANAGEMENT_NAME,
    };

    let script_name = script_path.file_name().unwrap().to_str().unwrap();
    let script_content = std::fs::read_to_string(script_path).unwrap();

    debug!("Running Script: {:?}", script_name);
    debug!("Content: {:?}", script_content);

    let mut copy_session = ExecSession::start(
        &config.endpoint,
        &config.address,
        config.port,
        &running_alloc.id,
        JOB_NAME,
        &["/bin/bash"],
    )
    .await
    .map_err(|e| RunError::StartingExecSession {
        ctx: "Copying script file",
        error: e,
    })?;

    copy_session
        .write_to_file(&script_content, &format!("/alloc/{}", script_name))
        .await
        .map_err(|e| RunError::Other(Cow::Borrowed("Writing File to ExecSession")))?;

    ExecSession::start(
        &config.endpoint,
        &config.address,
        config.port,
        &running_alloc.id,
        JOB_NAME,
        &["/bin/bash"],
    )
    .await
    .map_err(|e| RunError::StartingExecSession {
        ctx: "Setting up environment",
        error: e,
    })?
    .execute_command(
        &format!(
            "mkdir /alloc/builds; cd /alloc/builds; chmod +x /alloc/{}; exit 0;",
            script_name
        ),
        |_| {},
        |_| {},
    )
    .await
    .map_err(|e| RunError::Other(Cow::Borrowed("Executing command on ExecSession")))?;

    let mut run_session = ExecSession::start(
        &config.endpoint,
        &config.address,
        config.port,
        &running_alloc.id,
        job_name,
        &["/bin/bash", &format!("/alloc/{}", script_name)],
    )
    .await
    .map_err(|e| RunError::StartingExecSession {
        ctx: "Running loaded script",
        error: e,
    })?;

    let exit_code = run_session
        .read_logs(
            |msg| {
                print!("{}", msg);
            },
            |msg| {
                eprint!("{}", msg);
            },
        )
        .await
        .map_err(|e| RunError::Other(Cow::Borrowed("Reading logs from ExecSession")))?;

    let details = nomad_client
        .read_allocation(&running_alloc.id)
        .await
        .map_err(|e| RunError::Other(Cow::Borrowed("Reading Allocation Details")))?;

    if let Some(states) = details.task_states.get(JOB_NAME) {
        for event in states.events.iter() {
            if event.exit_code.unwrap_or(0) != 0 {
                println!(
                    "Task stopped with Non-Zero exit code: {:?}",
                    event.exit_code
                );

                return Ok(event.exit_code.unwrap());
            }
        }
    }

    Ok(exit_code)
}

/// Runs the Cleanup Stage for the Gitlab Runner
///
/// # Actions
/// * Deletes the previously created Batch Job for the Gitlab Job
pub async fn cleanup(config: &NomadConfig, info: &gitlab::JobInfo) {
    let nomad_client = config_to_client(config);
   
    println!("Removing Job: '{}'", info.job_id);
    match nomad_client.remove_job(&info.job_id).await {
        Ok(_) => {
            println!("Removed Job: '{}'", info.job_id);
        }
        Err(e) => {
            println!("Failed to remove job: '{}'", info.job_id);
        }
    };
}
