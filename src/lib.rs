use std::{path::Path, time::Duration};

use crate::nomad::{allocation, job};

mod gitlab;
mod nomad;

pub use gitlab::{CiEnv, JobInfo};

use futures_util::{stream::StreamExt, SinkExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};

const JOB_NAME: &str = "Job";
const MANAGEMENT_NAME: &str = "Manage";

/// The Nomad Configuration
///
/// This is needed to configure the correct Access to a Nomad Cluster
#[derive(Debug)]
pub struct NomadConfig {
    pub address: String,
    pub port: u16,
    pub datacenters: Vec<String>,
}

impl NomadConfig {
    /// Creates the Nomad Configuration with default starting Values and then overwrites these
    /// Values with values loaded from Environment Values
    ///
    /// # Environment Variables
    /// * `NOMAD_ADDR`: The Nomad address
    /// * `NOMAD_PORT`: The Nomad Port
    /// * `NOMAD_DATACENTER`: The Datacenter in which to run the Jobs
    pub fn load_with_defaults() -> Self {
        let mut raw = Self {
            address: "127.0.0.1".to_string(),
            port: 4646,
            datacenters: vec!["dc1".to_string()],
        };

        // Search for set environment variables
        for (key, value) in std::env::vars() {
            match key.as_str() {
                "NOMAD_ADDR" => {
                    raw.address = value;
                }
                "NOMAD_PORT" => {
                    if let Ok(port) = value.parse() {
                        raw.port = port;
                    }
                }
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
    gitlab::JobConfig {
        driver: gitlab::DriverInfo::new(),
        job_env: gitlab::JobInfo {
            job_id: format!("ci-{}", ci_env.ci_job_id),
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
pub async fn prepere(config: &NomadConfig, info: &gitlab::JobInfo, ci_env: &CiEnv) {
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
                        volumes: vec!["../alloc/:/mnt/alloc".to_string()],
                        work_dir: "/mnt/alloc".to_string(),
                        mounts: vec![],
                    },
                    env: [("GIT_SSL_NO_VERIFY".to_string(), "true".to_string())]
                        .into_iter()
                        .collect(),
                    resources: job::TaskResources {
                        cpu: 3000,
                        memory_mb: 1000,
                    },
                },
                job::Task {
                    name: MANAGEMENT_NAME.to_string(),
                    config: job::TaskConfig::Docker {
                        image: "gitlab/gitlab-runner:latest".to_string(),
                        entrypoint: vec!["/bin/bash".to_string()],
                        interactive: true,
                        volumes: vec!["../alloc/:/mnt/alloc".to_string()],
                        work_dir: "/mnt/alloc".to_string(),
                        mounts: vec![],
                    },
                    env: [("GIT_SSL_NO_VERIFY".to_string(), "true".to_string())]
                        .into_iter()
                        .collect(),
                    resources: job::TaskResources {
                        cpu: 100,
                        memory_mb: 500,
                    },
                },
            ],
        }],
    };

    let nomad_client = nomad::Client::new(config.address.clone(), config.port);

    let res_body = nomad_client.run_job(job_spec).await.unwrap();
    println!("Body: {:?}", res_body);

    loop {
        let eval_allocs = nomad_client
            .get_eval_allocations(&res_body.eval_id)
            .await
            .unwrap();

        let running = !eval_allocs.is_empty()
            && eval_allocs.iter().all(|alloc| {
                !alloc.task_states.is_empty()
                    && alloc
                        .task_states
                        .values()
                        .all(|state| state.state.eq_ignore_ascii_case("running"))
            });

        if running {
            break;
        }

        println!("Body: {:?}", eval_allocs);

        tokio::time::sleep(Duration::from_millis(1250)).await;
    }
}

/// Runs the Run Stage for the Gitlab Runner
pub async fn run(
    config: &NomadConfig,
    info: &gitlab::JobInfo,
    script_path: &Path,
    sub_stage: RunSubStage,
) -> Result<i32, ()> {
    let nomad_client = nomad::Client::new(config.address.clone(), config.port);

    let content = nomad_client
        .get_job_allocations(&info.job_id)
        .await
        .expect("Should be able to load Job Allocations");

    let running_alloc = match content.into_iter().find(|alloc| {
        alloc
            .task_states
            .iter()
            .all(|(_, state)| !state.failed && state.state == "running")
    }) {
        Some(a) => a,
        None => {
            todo!("No running allocation");
        }
    };

    let job_name = match sub_stage {
        RunSubStage::BuildScript | RunSubStage::AfterScript => JOB_NAME,
        _ => MANAGEMENT_NAME,
    };

    let script_name = script_path.file_name().unwrap().to_str().unwrap();
    let script_content = std::fs::read_to_string(script_path).unwrap();

    let mut copy_session = ExecSession::start(
        &config.address,
        config.port,
        &running_alloc.id,
        JOB_NAME,
        &["/bin/bash"],
    )
    .await
    .unwrap();
    copy_session
        .write_to_file(&script_content, &format!("/mnt/alloc/{}", script_name))
        .await
        .unwrap();

    ExecSession::start(
        &config.address,
        config.port,
        &running_alloc.id,
        JOB_NAME,
        &["/bin/bash"],
    )
    .await
    .unwrap()
    .execute_command(
        &format!(
            "mkdir /mnt/alloc/builds; cd /mnt/alloc/builds; chmod +x /mnt/alloc/{}; exit 0;",
            script_name
        ),
        |_| {},
        |_| {},
    )
    .await
    .unwrap();

    let mut run_session = ExecSession::start(
        &config.address,
        config.port,
        &running_alloc.id,
        job_name,
        &["/bin/bash", &format!("/mnt/alloc/{}", script_name)],
    )
    .await
    .unwrap();

    let exit_code = run_session
        .execute_command(
            &format!(""),
            |msg| {
                println!("{}", msg);
            },
            |msg| {
                eprintln!("{}", msg);
            },
        )
        .await
        .unwrap();

    Ok(exit_code)
}

/// Runs the Cleanup Stage for the Gitlab Runner
///
/// # Actions
/// * Deletes the previously created Batch Job for the Gitlab Job
pub async fn cleanup(config: &NomadConfig, info: &gitlab::JobInfo) {
    let client = reqwest::Client::new();

    let url = {
        let mut tmp = url_builder::URLBuilder::new();

        let route = format!("v1/job/{}", info.job_id);
        tmp.set_host(&config.address)
            .set_port(config.port)
            .set_protocol("http")
            .add_route(&route)
            .add_param("purge", "true");

        tmp.build()
    };

    let res = client.delete(url).send().await.unwrap();

    if !res.status().is_success() {
        todo!("Failed to delete Nomad job")
    }

    let raw_body = res.text().await.unwrap();
    println!("Response: {:?}", raw_body);
}

struct ExecSession {
    connection: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl ExecSession {
    pub async fn start(
        host: &str,
        port: u16,
        alloc_id: &str,
        task_name: &str,
        cmds: &[&str],
    ) -> Result<Self, ()> {
        let route = format!("v1/client/allocation/{}/exec", alloc_id);

        let cmd_string = serde_json::to_string(cmds).unwrap();
        let command_encoded = urlencoding::encode(&cmd_string);

        let mut ub = url_builder::URLBuilder::new();
        ub.set_protocol("ws")
            .set_host(host)
            .set_port(port)
            .add_route(&route)
            .add_param("command", &command_encoded)
            .add_param("task", task_name)
            .add_param("tty", "false");
        let ws_url = ub.build();

        let (ws_connection, response) = match tokio_tungstenite::connect_async(ws_url).await {
            Ok(c) => c,
            Err(e) => {
                match e {
                    tokio_tungstenite::tungstenite::Error::Http(resp) => {
                        let body = resp.body().clone().unwrap();
                        let string = String::from_utf8(body.to_vec()).unwrap();

                        println!("Response: {:?}", string);
                        todo!("{:?}", resp);
                    }
                    other => todo!("{:?}", other),
                };
            }
        };

        if !response.status().is_informational() {
            println!("Websocket Response: {:?}", response);

            todo!("Establishing Websocket Connection was not successful");
        }

        Ok(Self {
            connection: ws_connection,
        })
    }

    pub async fn execute_command<SO, SE>(
        &mut self,
        command: &str,
        mut stdout: SO,
        mut stderr: SE,
    ) -> Result<i32, ()>
    where
        SO: FnMut(String),
        SE: FnMut(String),
    {
        let request = allocation::ExecRequestFrame {
            stdin: allocation::ExecData::data(&format!("{}\n", command)),
        };

        self.connection
            .send(Message::Text(serde_json::to_string(&request).unwrap()))
            .await
            .unwrap();

        let mut exit_code = 0;

        while let Some(msg_res) = self.connection.next().await {
            let msg = match msg_res {
                Ok(m) => m,
                Err(e) => {
                    panic!("Error: {:?}", e);
                }
            };

            let frame: allocation::ExecResponseFrame = match msg {
                Message::Text(txt) => serde_json::from_str(&txt).unwrap(),
                Message::Close(_close_frame) => {
                    // println!("{:?}", _close_frame);

                    break;
                }
                other => panic!("Unexpected Message: {:?}", other),
            };

            if let Some(tmp) = &frame.stdout {
                stdout(tmp.decode_data());
            }
            if let Some(tmp) = &frame.stderr {
                stderr(tmp.decode_data());
            }

            if let Some(res) = frame.result {
                exit_code = res.exit_code;
            }
        }

        Ok(exit_code)
    }

    pub async fn write_to_file(&mut self, content: &str, path: &str) -> Result<i32, ()> {
        let cmd = format!(
            "echo \"{}\" > {}; exit 0",
            content.replace('"', "\\\"").replace('\\', "\\\\"),
            path
        );

        self.execute_command(&cmd, |_| {}, |_| {}).await
    }
}
