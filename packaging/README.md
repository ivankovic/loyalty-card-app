# Running it

The app is one container: a static binary with the web app built in, run as nobody on an empty
image. Its state is one SQLite file, `/data/loyalty.db`. It is meant to sit behind nginx (or any
proxy) that terminates TLS: a phone only lets a web app use the camera over HTTPS.

- `docker/Dockerfile`: the image. Build it from the repository root.
- `compose/loyalty.yml`: the Compose service, with the hardening the image is built for
  (read-only, no capabilities, an init, a memory limit).
- `nginx/loyalty.conf`: the site.

## First run

1. Make the data folder, owned by the user the container runs as:

       sudo install -d -o 65534 -g 65534 -m 700 /srv/domaci/loyalty

2. Add the service from `compose/loyalty.yml` to `docker-compose.yml`, with your origin, and
   start it:

       sudo docker compose up -d --build loyalty

3. Add the site from `nginx/loyalty.conf` to nginx, with a certificate for the name, then
   `nginx -t` and reload.

4. Make your household and your own invite:

       sudo docker exec loyalty /loyalty household add Home
       sudo docker exec loyalty /loyalty invite --household Home marko --admin

   Open the printed link on your phone, choose a password, and use "Add to Home Screen"
   (Safari's Share menu on an iPhone; Chrome's menu on Android). Invite the others from the
   app's settings, or with the same command.

## Updating

    git pull && sudo docker compose up -d --build loyalty

The database is migrated when the new version opens it. A phone picks up the new app the next
time it opens it online.

## Backups

The database is one file, but copying it while the server writes can catch it half-written.
`backup` writes a consistent copy, safe while the server runs:

    sudo docker exec loyalty /loyalty backup /data/backup.db

Run that before your file backup copies `/srv/domaci/loyalty` (it refuses to overwrite, so
remove the previous copy first). Restoring is stopping the container and putting a copy back as
`loyalty.db`.

## Other commands

    loyalty user list                  everyone, and who has not set a password yet
    loyalty user reset-password NAME   a link to choose a new password (a lost phone)
    loyalty user remove NAME           their private cards go; their other cards become the family's
    loyalty household list

## Behind a client certificate (optional)

If the server already asks devices for a client certificate (for example from Friends and Family
CA), the site can do the same: see the `BEGIN ffca` block in `nginx/loyalty.conf`. The app does
not need it; its own sign-in is enough.
