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
    config_args = [ "--config", "/local/config.yaml", "config" ]
    config_exec_timeout = 200

    prepare_exec = "/bin/nomad-runner"
    prepare_args = [ "--config", "/local/config.yaml", "prepare" ]
    prepare_exec_timeout = 200

    run_exec = "/bin/nomad-runner"
    run_args = [ "--config", "/local/config.yaml", "run" ]

    cleanup_exec = "/bin/nomad-runner"
    cleanup_args = [ "--config", "/local/config.yaml", "cleanup" ]
    cleanup_exec_timeout = 200

    graceful_kill_timeout = 200
    force_kill_timeout = 200
EOF
        destination = "local/runner-config.toml"
        change_mode = "restart"
      }

      template {
        data = <<EOF
datacenters:
  - 'dc1'
EOF
        destination = "local/config.yaml"
        change_mode = "restart"
      }

      config {
        image = "ghcr.io/lol3rrr/nomad-runner:latest"

        args = ["run", "-c", "${NOMAD_TASK_DIR}/runner-config.toml"]
      }

      env {
        NOMAD_DATACENTER = "dc1"
        NOMAD_SOCKET     = "${NOMAD_SECRETS_DIR}/api.sock"
        NOMAD_TOKEN      = "${NOMAD_TOKEN}"
      }
    }
  }
}
