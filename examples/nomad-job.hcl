job "gitlab" {
  datacenters = ["dc1"]

  type = "service"

  group "runner" {
    count = 1

    task "runner" {
      driver = "docker"

      template {
        data = <<EOF
[[runners]]
  name = "testing"
  url = "https://gitlab.com"
  token = "{{ with nomadVar "nomad/jobs" }}{{ .gitlab_runner_key }}{{ end }}"
  executor = "custom"
  builds_dir = "/alloc/builds"
  cache_dir = "/alloc/cache"
  shell = "bash"
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
EOF
        destination = "local/config.toml"
        change_mode = "restart"
      }

      config {
        image = "ghcr.io/lol3rrr/nomad-runner:latest"

        args = ["run", "-c", "${NOMAD_TASK_DIR}/config.toml"]
      }

      env {
        NOMAD_ADDR       = "nomad.service.consul"
        NOMAD_PORT       = "4646"
        NOMAD_DATACENTER = "dc1"
      }
    }
  }
}
