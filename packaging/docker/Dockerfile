# This file is part of Loyalty Card App.
#
# Copyright (C) 2026 Marko Ivankovic
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU Affero General Public License as published
# by the Free Software Foundation, version 3 of the License.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
# GNU Affero General Public License for more details.
#
# You should have received a copy of the GNU Affero General Public License
# along with this program. If not, see <https://www.gnu.org/licenses/>.

# The app as a container: one static binary, with the web app built into it, on an empty
# image, run as nobody. Its only state is the database in /data.
#
# Build from the repository root:
#     docker build -f packaging/docker/Dockerfile -t loyalty-card-app .
#
# Run it with an init (Compose's `init: true`, `docker run --init`), a writable /data and
# LOYALTY_ORIGIN set to where people open it - see packaging/README.md.

# Both build images are pinned by digest, which dependabot moves.

# The web app. Only its runtime dependencies are installed: the bundler is Bun itself.
FROM docker.io/oven/bun:1.4.3-alpine@sha256:629e17411f1f129dbec3af78d5af9c9f2a937435c80349437206c6b0b7422373 AS web
WORKDIR /src/web
COPY web/package.json web/bun.lock ./
RUN bun install --frozen-lockfile --production --ignore-scripts
COPY web/ ./
RUN bun run build.ts

# The server, with web/dist embedded. musl links statically: the binary needs nothing from the
# image it runs in. The build fetches nothing but the crates Cargo.lock names.
FROM docker.io/library/rust:1-alpine@sha256:a96ea6d18d4062e38f16cfbadd8b4541d622f2527dd0a5eca1fb36d301da4e88 AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock build.rs ./
COPY src/ src/
COPY --from=web /src/web/dist web/dist
RUN cargo build --release --locked --bin loyalty

FROM scratch
COPY --from=build /src/target/release/loyalty /loyalty
USER 65534:65534
ENV LOYALTY_DB=/data/loyalty.db
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s CMD ["/loyalty", "healthz"]
ENTRYPOINT ["/loyalty"]
CMD ["serve", "--listen", "0.0.0.0:8080"]
