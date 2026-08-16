# Security Policy

## Supported versions

Security fixes are applied to the latest released minor line. During the 0.x series, users should
upgrade to the newest release before reporting an issue as unfixed.

## Reporting

For a private vulnerability report, use GitHub's private vulnerability reporting feature when it is
enabled for the repository. Do not include passwords, browser cookies, SSH keys, API tokens or other
secrets in issue attachments.

## Security model

`niripipd` is a per-user session daemon. It:

- must not run as root;
- listens only on a Unix socket under `$XDG_RUNTIME_DIR/niri-pip/`;
- creates the runtime directory as `0700` and socket as `0600`;
- does not bind TCP/UDP ports;
- does not execute shell strings from `config.toml`;
- sends only a narrow verified set of Niri IPC actions;
- stores non-secret geometry preferences under XDG state storage.

The daemon controls windows in the same user's compositor session, so another process running as the
same Unix user is already within the same desktop trust boundary. The socket permissions prevent
cross-user access on a correctly configured runtime directory.
