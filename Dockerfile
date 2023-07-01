# Based on https://kerkour.com/rust-small-docker-image
FROM rust:1.70 AS builder

WORKDIR /runner/nomad-runner

# Create appuser
ENV USER=runner
ENV UID=10001

RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}"

COPY ./ /runner/nomad-runner

RUN cargo build --release

FROM gitlab/gitlab-runner:ubuntu-v16.1.0

# Copy our build
COPY --from=builder /runner/nomad-runner/target/release/nomad-runner /bin/nomad-runner
