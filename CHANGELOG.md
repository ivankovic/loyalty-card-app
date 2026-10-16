# Changelog

All notable changes are recorded here, in the format of
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow
[Semantic Versioning](https://semver.org/); before 1.0, a minor version may change the API or the
command line, and a patch version does not. The database is migrated forward by every release.

## [Unreleased]

### Added

- Households, people, invites and sign-in, with a throttle on wrong passwords.
- Cards held by a person or by the family, with a private switch for a person's own.
- Thirteen barcode symbologies, checked when saved and drawn as crisp SVG.
- Scanning with the camera, or from a photo; front and back photos, shrunk on the phone.
- The till view: the code full size on white, the screen kept on.
- Offline: the app and the last cards open with no signal.
- The container image, the Compose service and the nginx site; `loyalty backup`.
