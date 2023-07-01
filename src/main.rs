use std::{collections::HashMap, path::PathBuf};

use clap::{Parser, Subcommand};
use nomad_runner::{CiEnv, JobInfo, NomadConfig};

/// A custom runner for Gitlab that allows you to run/schedule the Gitlab Jobs on a Nomad Cluster.
///
/// This allows for greater scalability when running a single Gitlab Runner instance itself, as you
/// can then utilize the resources of the entire nomad cluster instead of just a single machine.
#[derive(Debug, Parser)]
#[clap(name = "Nomad-Runner", version)]
pub struct App {
    #[clap(flatten)]
    ci_env: CiEnv,

    #[clap(flatten)]
    job_env: Option<JobInfo>,

    #[clap(subcommand)]
    command: Command,
}

/// The Commands supported by the custom Runner
#[derive(Debug, Subcommand)]
enum Command {
    /// Run the 'Config' Stage of the Gitlab Runner
    Config,
    /// Prepares the Environment for executing the given Job on the Nomad Cluster.
    ///
    /// This is executed as part of the 'Prepare' Stage of the Gitlab Runner
    Prepare,
    /// Executes the actual Scripts for the Job
    Run {
        /// The Path to the Script to run
        script_path: PathBuf,
        /// The Substage of the 'Run' Stage, where this is part of
        #[arg(value_parser = clap::value_parser!(nomad_runner::RunSubStage))]
        substage: nomad_runner::RunSubStage,
    },
    /// Cleans up all the allocated Ressources from this Job
    Cleanup,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    for arg in std::env::args_os() {
        // eprintln!("[ARG] {:?}", arg);
    }
    for env in std::env::vars_os() {
        // eprintln!("[ENV] {:?}", env);
    }

    let args = App::parse();

    let env_values = args.ci_env;

    let nomad_config = NomadConfig {
        address: "192.168.10.8".to_owned(),
        port: 5646,
        datacenters: vec!["aachen".to_string()],
    };

    let all_envs: HashMap<String, String> = std::env::vars()
        .filter(|(key, _)| {
            key.starts_with("CUSTOM_ENV_CI_")
                || key.starts_with("GITLAB_")
                || key.starts_with("CI_")
        })
        .collect();

    match args.command {
        Command::Config => {
            let job_config = nomad_runner::config(&env_values);
            let job_config_json =
                serde_json::to_string(&job_config).expect("Job Config should be serializable");

            println!("{}", job_config_json);
        }
        Command::Prepare => {
            let job_env = args.job_env.expect("Job Environment should be set");

            nomad_runner::prepere(&nomad_config, &job_env, &env_values, &all_envs).await;
        }
        Command::Run {
            script_path,
            substage,
        } => {
            let job_env = args.job_env.expect("Job Environment should be set");

            nomad_runner::run(&nomad_config, &job_env, &script_path, substage).await;
        }
        Command::Cleanup => {
            let job_env = args.job_env.expect("Job Environment should be set");

            println!("Run Cleanup");

            // TODO
            // Only for testing
            // nomad_runner::cleanup(&nomad_config, &job_env).await;
        }
    };
}
