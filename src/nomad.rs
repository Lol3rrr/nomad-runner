/// All the Nomad Job related definitions
pub mod job {
    use std::collections::HashMap;

    use serde::{Deserialize, Serialize};

    /// The Job Specification
    #[derive(Debug, Serialize)]
    pub struct Spec {
        /// The Datacenters in which this Job should run
        #[serde(rename = "Datacenters")]
        pub datacenters: Vec<String>,

        /// The ID of the Job
        #[serde(rename = "ID")]
        pub id: String,

        /// The Type of the Job
        #[serde(rename = "Type")]
        pub ty: Ty,

        /// The Task Groups that are part of this Job
        #[serde(rename = "TaskGroups")]
        pub task_groups: Vec<TaskGroup>,
    }

    /// The Types of Jobs that can be run
    #[derive(Debug, Serialize)]
    pub enum Ty {
        /// Service Jobs should run continuously and are restarted after stopping on their own.
        #[serde(rename = "service")]
        Service,
        /// Batch Jobs are only restarted if they exit with a failure, indicated by an exit code
        /// other than 0
        #[serde(rename = "batch")]
        Batch,
    }

    /// A single Task Group for a Job
    #[derive(Debug, Serialize)]
    pub struct TaskGroup {
        /// The Name of the Task Group
        #[serde(rename = "Name")]
        pub name: String,

        /// The Network Configuration for the entire Task Group
        #[serde(rename = "Networks")]
        pub networks: Vec<serde_json::Value>,

        /// The Service Configuration for the entire Task Group
        #[serde(rename = "Services")]
        pub services: Vec<serde_json::Value>,

        /// The Tasks that are part of this Task Group
        #[serde(rename = "Tasks")]
        pub tasks: Vec<Task>,
    }

    /// A single Task that is part of a Task Group
    #[derive(Debug, Serialize)]
    pub struct Task {
        /// The Name of the Task itself
        #[serde(rename = "Name")]
        pub name: String,

        /// The Driver Configuration for the Task
        #[serde(flatten)]
        pub config: TaskConfig,

        /// The Resources needed by the Task
        #[serde(rename = "Resources")]
        pub resources: TaskResources,

        /// The Environment Variables for the Job
        #[serde(rename = "Env")]
        pub env: HashMap<String, String>,
    }

    #[derive(Debug, Serialize)]
    #[serde(tag = "Driver", content = "Config")]
    pub enum TaskConfig {
        /// The Docker Driver Configuration
        #[serde(rename = "docker")]
        Docker {
            /// The Image to use
            image: String,
            /// The Command to execute inside of the Container
            entrypoint: Vec<String>,
            /// Whether or not the Container should be run in interactive mode or not
            interactive: bool,
            /// Volumes to mount
            volumes: Vec<String>,
            /// The Working Directory inside of the Container
            work_dir: String,
            mounts: Vec<DockerMount>,
        },
    }

    #[derive(Debug, Serialize)]
    pub struct DockerMount {
        #[serde(rename = "type")]
        pub ty: String,
        pub source: String,
        pub target: String,
        pub readonly: bool,
    }

    /// The Resources requested for a given Task
    #[derive(Debug, Serialize)]
    pub struct TaskResources {
        /// The CPU Capacity that is needed in MhZ
        #[serde(rename = "CPU")]
        pub cpu: usize,

        /// The Memory Capactity needed in MB
        #[serde(rename = "MemoryMB")]
        pub memory_mb: usize,
    }

    #[derive(Debug, Serialize)]
    pub struct RunRequest {
        #[serde(rename = "Job")]
        pub job: Spec,
    }

    #[derive(Debug, Deserialize)]
    pub struct RunResponse {
        #[serde(rename = "EvalID")]
        pub eval_id: String,
        #[serde(rename = "EvalCreateIndex")]
        pub eval_create_index: usize,
        #[serde(rename = "JobModifyIndex")]
        pub job_modify_index: usize,
        #[serde(rename = "Warnings")]
        pub warnings: String,
        #[serde(rename = "Index")]
        pub index: usize,
        #[serde(rename = "LastContact")]
        pub last_contact: usize,
        #[serde(rename = "KnownLeader")]
        pub known_leader: bool,
    }
}

pub mod allocation {
    use std::collections::HashMap;

    use base64::Engine as _;
    use serde::{Deserialize, Deserializer, Serialize};

    pub type ListJobAllocationsResponse = Vec<Spec>;

    #[derive(Debug, Deserialize)]
    pub struct Spec {
        #[serde(rename = "ID")]
        pub id: String,

        #[serde(rename = "JobID")]
        pub job_id: String,

        #[serde(rename = "Name")]
        pub name: String,

        #[serde(rename = "TaskGroup")]
        pub task_group: String,
        #[serde(rename = "Namespace")]
        pub namespace: String,

        #[serde(rename = "NodeID")]
        pub node_id: String,
        #[serde(rename = "NodeName")]
        pub node_name: String,

        #[serde(rename = "DesiredStatus")]
        pub desired_status: String,

        #[serde(rename = "ClientStatus")]
        pub client_status: String,

        #[serde(
            rename = "TaskStates",
            default,
            deserialize_with = "nullable_taskstates"
        )]
        pub task_states: HashMap<String, TaskState>,

        #[serde(flatten)]
        pub rest: HashMap<String, serde_json::Value>,
    }

    fn nullable_taskstates<'de, D>(deserializer: D) -> Result<HashMap<String, TaskState>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let tmp = Option::deserialize(deserializer)?;
        Ok(tmp.unwrap_or_default())
    }

    #[derive(Debug, Deserialize)]
    pub struct TaskState {
        #[serde(rename = "State")]
        pub state: String,

        #[serde(rename = "Failed")]
        pub failed: bool,
    }

    #[derive(Debug, Serialize)]
    pub struct ExecRequestFrame {
        pub stdin: ExecData,
    }

    #[derive(Debug, Deserialize)]
    pub struct ExecResponseFrame {
        pub stdout: Option<ExecData>,
        pub stderr: Option<ExecData>,

        #[serde(default)]
        pub exited: bool,
        pub result: Option<ExecResult>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct ExecResult {
        #[serde(default)]
        pub exit_code: i32,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct ExecData {
        #[serde(default)]
        data: String,
        #[serde(default)]
        close: bool,
    }

    impl ExecData {
        pub fn data(data: &str) -> Self {
            Self {
                data: base64::engine::general_purpose::STANDARD.encode(data),
                close: false,
            }
        }

        pub fn decode_data(&self) -> String {
            // TODO
            // Figure out error handling for this

            base64::engine::general_purpose::STANDARD
                .decode(&self.data)
                .map_err(|e| ())
                .and_then(|d| String::from_utf8(d).map_err(|e| ()))
                .unwrap()
        }
    }
}

pub mod evaluations {
    pub type ListAllocationsResponse = Vec<super::allocation::Spec>;
}

pub mod events {
    use std::{collections::HashMap, path::Display};

    use serde::{Deserialize, Serialize};

    use super::allocation;

    #[derive(Debug, Deserialize)]
    pub struct Event {
        #[serde(rename = "FilterKeys", default)]
        pub filter_keys: Option<Vec<String>>,
        #[serde(rename = "Index")]
        pub index: usize,
        #[serde(rename = "Key")]
        pub key: String,
        #[serde(rename = "Namespace")]
        pub namespace: String,
        #[serde(rename = "Payload")]
        pub payload: Payload,
        #[serde(rename = "Topic")]
        pub topic: Topic,
        #[serde(rename = "Type")]
        pub ty: String,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub enum Topic {
        #[serde(rename = "*")]
        Any,
        ACLToken,
        ACLPolicy,
        ACLRoles,
        Allocation,
        Job,
        Evaluation,
        Deployment,
        Node,
        NodeDrain,
        NodePool,
        Service,
    }

    impl std::fmt::Display for Topic {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Any => write!(f, "*"),
                other => std::fmt::Debug::fmt(&self, f),
            }
        }
    }

    #[derive(Debug, Deserialize)]
    pub struct Payload {
        #[serde(rename = "Allocation")]
        pub allocation: Option<allocation::Spec>,
        #[serde(flatten)]
        pub rest: HashMap<String, serde_json::Value>,
    }

    #[derive(Debug, Deserialize)]
    pub struct RawEventStreamMessage {
        #[serde(rename = "Events", default)]
        pub events: Vec<Event>,
    }
}

pub struct Client {
    addr: String,
    port: u16,
    http_client: reqwest::Client,
}

#[derive(Debug)]
pub enum ClientRequestError {
    SendingRequest(reqwest::Error),
    ErrorResponse { status_code: reqwest::StatusCode },
    InvalidResponse(reqwest::Error),
}

impl Client {
    pub fn new(addr: String, port: u16) -> Self {
        Self {
            addr,
            port,
            http_client: reqwest::Client::new(),
        }
    }

    pub async fn get_job_allocations(
        &self,
        job_id: &str,
    ) -> Result<allocation::ListJobAllocationsResponse, ClientRequestError> {
        let url = {
            let mut tmp = url_builder::URLBuilder::new();

            let route = format!("v1/job/{}/allocations", job_id);
            tmp.set_host(&self.addr)
                .set_port(self.port)
                .set_protocol("http")
                .add_route(&route);

            tmp.build()
        };

        let res = self
            .http_client
            .get(url)
            .send()
            .await
            .map_err(ClientRequestError::SendingRequest)?;

        if !res.status().is_success() {
            return Err(ClientRequestError::ErrorResponse {
                status_code: res.status(),
            });
        }

        res.json()
            .await
            .map_err(ClientRequestError::InvalidResponse)
    }

    pub async fn run_job(&self, spec: job::Spec) -> Result<job::RunResponse, ClientRequestError> {
        let job_spec_json = job::RunRequest { job: spec };

        let url = {
            let mut tmp = url_builder::URLBuilder::new();

            tmp.set_host(&self.addr)
                .set_port(self.port)
                .set_protocol("http")
                .add_route("v1/jobs");

            tmp.build()
        };

        let res = self
            .http_client
            .post(url)
            .json(&job_spec_json)
            .send()
            .await
            .map_err(ClientRequestError::SendingRequest)?;

        if !res.status().is_success() {
            let status = res.status();

            println!("Response: {:?}", res);
            let body = res.bytes().await.expect("");

            let body_str = String::from_utf8(body.to_vec()).unwrap();
            println!("Body: {:?}", body_str);

            return Err(ClientRequestError::ErrorResponse {
                status_code: status,
            });
        }

        res.json()
            .await
            .map_err(ClientRequestError::InvalidResponse)
    }

    pub async fn get_eval_allocations(
        &self,
        eval: &str,
    ) -> Result<evaluations::ListAllocationsResponse, ClientRequestError> {
        let url = {
            let mut tmp = url_builder::URLBuilder::new();

            tmp.set_host(&self.addr)
                .set_port(self.port)
                .set_protocol("http")
                .add_route(&format!("v1/evaluation/{}/allocations", eval));

            tmp.build()
        };

        let res = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(ClientRequestError::SendingRequest)?;

        if !res.status().is_success() {
            return Err(ClientRequestError::ErrorResponse {
                status_code: res.status(),
            });
        }

        res.json()
            .await
            .map_err(ClientRequestError::InvalidResponse)
    }

    /// Get an Event Stream from Nomad
    pub async fn events(
        &self,
        after: usize,
        topics: Option<&[events::Topic]>,
    ) -> Result<tokio::sync::mpsc::Receiver<events::Event>, ClientRequestError> {
        let url = {
            let mut tmp = url_builder::URLBuilder::new();

            tmp.set_host(&self.addr)
                .set_port(self.port)
                .set_protocol("http")
                .add_param("index", &format!("{}", after))
                .add_route("v1/event/stream");

            if let Some(topics) = topics {
                for t in topics {
                    tmp.add_param("topic", &format!("{}:*", t));
                }
            }

            tmp.build()
        };

        let mut res = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(ClientRequestError::SendingRequest)?;

        let (tx, rx) = tokio::sync::mpsc::channel(8);

        // Read from the Stream from the API, processes the Raw Data and enqueue the individual
        // Events received
        tokio::spawn(async move {
            let mut buffer = Vec::new();

            while let Ok(Some(chunk)) = res.chunk().await {
                buffer.extend(chunk);

                // Continue to find the newline seperating different JSON payloads
                while let Some(idx) = buffer
                    .iter()
                    .enumerate()
                    .find(|(_, c)| **c == b'\n')
                    .map(|(i, _)| i)
                {
                    let inner: Vec<u8> = buffer.drain(0..idx).collect();
                    buffer.remove(0);

                    let raw_events: events::RawEventStreamMessage =
                        serde_json::from_slice(&inner).unwrap();
                    // println!("Raw-Event: {:#?}", raw_events);

                    for ev in raw_events.events {
                        if let Err(e) = tx.send(ev).await {
                            return;
                        }
                    }
                }
            }
        });

        Ok(rx)
    }
}
