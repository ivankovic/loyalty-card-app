# Running it

The app is one container: a static binary with the web app built in, run as nobody on an empty
image. Its state is one SQLite file, `/data/loyalty.db`. It is meant to sit behind nginx (or any
proxy) that terminates TLS: a phone only lets a web app use the camera over HTTPS.

Each release is published as `ghcr.io/ivankovic/loyalty-card-app`, for amd64 and arm64 (a
Raspberry Pi 4 or 5 runs it). Tags: the exact version (`0.1.0`), the newest patch of a minor
version (`0.1`), and `latest`.

- `compose/loyalty.yml`: the Compose service, with the hardening the image is built for
  (read-only, no capabilities, an init, a memory limit).
- `nginx/loyalty.conf`: the site.
- `docker/Dockerfile`: the image, to build it yourself. Build it from the repository root.

## What you need

- Docker with Compose (or Podman).
- A name for it, such as `cards.example.org`, with a TLS certificate.
- nginx in front of it, in the same Compose project or on the host.

The server needs under 30 MB of memory; the limit in the Compose file is 128 MB.

## First run

1. Make the data folder, owned by the user the container runs as:

       sudo install -d -o 65534 -g 65534 -m 700 /srv/loyalty

2. Add the service from `compose/loyalty.yml` under `services:` in your `docker-compose.yml`.
   Set `LOYALTY_ORIGIN` to the address people will open, and put it on a network your nginx
   container shares. Start it:

       sudo docker compose up -d loyalty

3. Add the site from `nginx/loyalty.conf` to nginx with your name and certificate, then
   `nginx -t` and reload.

4. Make your household and your own invite:

       sudo docker exec loyalty /loyalty household add Home
       sudo docker exec loyalty /loyalty invite --household Home alice --admin

   Open the printed link on your phone, choose a password, and use "Add to Home Screen"
   (Safari's Share menu on an iPhone; Chrome's menu on Android). Invite the others from the
   app's settings, or with the same command.

### With nginx on the host

When nginx is not a container, publish the app's port on the loopback interface only:

    ports:
      - "127.0.0.1:8080:8080"

drop `networks:` from the service, and in the site replace the `resolver` and `set` lines and
`$loyalty` with `http://127.0.0.1:8080`.

### Any other proxy

The app needs three things from it: TLS, the `Host` header passed through, and the client's
address in `X-Real-IP` (the sign-in throttle counts by it; `LOYALTY_TRUST_PROXY` makes the app
read it). Allow request bodies of 4 MB, for the photos. Don't log `/i/` and `/api/invite/`: an
invite link carries its secret.

## Updating

Read the [changelog](../CHANGELOG.md) first. Then:

    sudo docker compose pull loyalty && sudo docker compose up -d loyalty

The database is migrated when the new version opens it, so take a backup first (below). A phone
picks up the new app the next time it opens it online.

Building from a checkout instead: `git pull && sudo docker compose up -d --build loyalty`.

## Backups

The database is one file, but copying it while the server writes can catch it half-written.
`backup` writes a consistent copy, safe while the server runs:

    sudo docker exec loyalty /loyalty backup /data/backup.db

Run that before your file backup copies `/srv/loyalty` (it refuses to overwrite, so remove the
previous copy first). Restoring is stopping the container and putting a copy back as
`loyalty.db`.

## Other commands

    loyalty user list                  everyone, and who has not set a password yet
    loyalty user reset-password NAME   a link to choose a new password (a lost phone)
    loyalty user remove NAME           their private cards go; their other cards become the family's
    loyalty household list

Run them as `sudo docker exec loyalty /loyalty …`; `--help` on any of them says more.

## Behind a client certificate (optional)

If the server already asks devices for a client certificate, the site can do the same: see the
`BEGIN ffca` block in `nginx/loyalty.conf`. The app does not need it; its own sign-in is enough.
