# Nomad Runner
A custom gitlab runner for running gitlab runs on a nomad cluster

## Usage
Using the docker image: `ghcr.io/lol3rrr/nomad-runner:latest`
Example Nomad job found in `examples/nomad-job.hcl`

Configuration for the runner itself:
```
[[runners]]
  executor = "custom"
  builds_dir = "/mnt/alloc/builds"
  cache_dir = "/mnt/alloc/cache"
  [runners.custom]
    config_exec = "/bin/nomad-runner"
    config_args = [ "config" ]
    config_exec_timeout = 200

    prepare_exec = "/bin/nomad-runner"
    prepare_args = [ "prepare" ]
    prepare_exec_timeout = 200

    run_exec = "/bin/nomad-runner"
    run_args = [ "run" ]

    cleanup_exec = "/bin/nomad-runner"
    cleanup_args = [ "cleanup" ]
    cleanup_exec_timeout = 200

    graceful_kill_timeout = 200
    force_kill_timeout = 200
```

## Architecture
To run a Gitlab Job, it starts a new Job with 2 Tasks consisting of one Management Task and one
Job Task. The Job Task is used for running all the user specified Commands and the Management Task
is used for running all the Management related commands, like cloning the repo, download/uploading
the artifacts etc.
