# Security

## Supported versions

Only the latest release is supported. Before 1.0 that means the latest minor version.

## Reporting a vulnerability

Please report it privately, through GitHub's private vulnerability reporting on this repository
(the "Security" tab, "Report a vulnerability"), or by email to
[marko@ivankovic.me](mailto:marko@ivankovic.me). Expect an answer within a week. Please do not
open a public issue for it.

## What counts

The app holds a household's cards and photos and is reachable from the internet. Things that
count as vulnerabilities:

- Reading or changing another household's cards or photos, or another person's private card.
- Signing in without the password, or keeping a session after a password reset.
- Getting around the sign-in throttle.
- Script injection: anything that runs script in the app's origin.
- Anything that makes the server crash or stop answering from a single request.

Things that do not:

- What someone who can run commands on the server, or read its database, can do.
- Attacks that need a person to install a malicious browser extension or profile.
