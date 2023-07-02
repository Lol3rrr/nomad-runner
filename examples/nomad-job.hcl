job "gitlab" {
  datacenters = ["dc1"]

  type = "service"

  group "runner" {
    count = 1

    task "runner" {
      driver = "docker"

      config {
        image = "ghcr.io/lol3rrr/nomad-runner:latest"

        args = ["run"]
      }

      env {
        NOMAD_ADDR       = "nomad.service.consul"
        NOMAD_PORT       = "4646"
        NOMAD_DATACENTER = "dc1"
      }
    }
  }
}
