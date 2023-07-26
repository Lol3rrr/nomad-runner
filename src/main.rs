use std::path::PathBuf;

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
    env_logger::init();

    let build_failure_exit_code: i32 = std::env::var("BUILD_FAILURE_EXIT_CODE")
        .map_err(|_| ())
        .and_then(|p| p.parse().map_err(|_| ()))
        .unwrap_or(-1);

    let args = App::parse();

    let env_values = args.ci_env;

    let nomad_config = NomadConfig::load_with_defaults();

    match args.command {
        Command::Config => {
            let job_config = nomad_runner::config(&env_values);
            let job_config_json =
                serde_json::to_string(&job_config).expect("Job Config should be serializable");

            println!("{}", job_config_json);
        }
        Command::Prepare => {
            let job_env = args.job_env.expect("Job Environment should be set");

            nomad_runner::prepare(&nomad_config, &job_env, &env_values).await;
        }
        Command::Run {
            script_path,
            substage,
        } => {
            let job_env = args.job_env.expect("Job Environment should be set");

            match nomad_runner::run(&nomad_config, &job_env, &script_path, substage).await {
                Ok(exit_code) => {
                    if exit_code != 0 {
                        eprintln!("Exited with Code: {}", exit_code);
                        std::process::exit(build_failure_exit_code);
                    }
                }
                Err(e) => {}
            };
        }
        Command::Cleanup => {
            let job_env = args.job_env.expect("Job Environment should be set");

            nomad_runner::cleanup(&nomad_config, &job_env).await;
        }
    };
}
