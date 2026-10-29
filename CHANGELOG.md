# Changelog

All notable changes are recorded here, in the format of
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow
[Semantic Versioning](https://semver.org/); before 1.0, a minor version may change the API or the
command line, and a patch version does not. The database is migrated forward by every release.

## [Unreleased]

### Added

- Typing a code by hand guesses its kind from what is typed, instead of needing it picked
  first.

## [0.1.0] - 2026-10-28

### Added

- Households, people, invites and sign-in, with a throttle on wrong passwords.
- Cards held by a person or by the family, with a private switch for a person's own.
- Thirteen barcode symbologies, checked when saved and drawn as crisp SVG.
- Scanning with the camera, or from a photo; front and back photos, shrunk on the phone.
- The till view: the code full size on white, the screen kept on.
- Offline: the app and the last cards open with no signal.
- The container image, published on GHCR for amd64 and arm64; the Compose service and the
  nginx site; `loyalty backup`.
- Light Academia and Dark Academia colours, following the phone's light or dark setting, or
  chosen per device in Settings.
- A home-screen icon in the Academia colours: a parchment card on a gold one, on leather.
- An invite link cut short when copied says so, instead of showing the sign-in page.

### Fixed

- Sideways, the code covered the button that turned it, leaving no way back on an iPhone; there
  is now a "Turn back" button, and a tap on the code turns it back too. The sideways code is
  centred instead of running off the right edge.
