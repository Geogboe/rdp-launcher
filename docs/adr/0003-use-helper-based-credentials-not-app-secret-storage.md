# ADR 0003: Use Helper-Based Credentials, Not App Secret Storage

## Status

Accepted

## Context

The product needs a better credential experience than stock MSTSC, but the app should not become a password vault. The desired behavior is closer to `SSH_ASKPASS` or a helper protocol that can integrate with an external password manager or script.

Windows Credential Manager exists and MSTSC is compatible with Windows credential flows, but using it as the app's primary secret store would blur the boundary between launcher and secret manager.

## Decision

Use an external helper executable contract for credential retrieval. The app sends a request to the helper at launch time and receives transient credentials back. The app may bridge those credentials into Windows Credential Manager when needed for MSTSC compatibility, but it will not persist secrets in its own database or config.

## Consequences

- The app stays vendor-neutral with respect to password managers.
- Enterprises and individual users can swap helpers without changing the app.
- The security boundary is clearer: the app is an orchestrator, not a vault.
- Launch-time complexity increases because the app must handle helper failures, TTLs, and cleanup.
- Windows policy may constrain what can be done with saved or delegated credentials, so diagnostics and warnings will matter.

## References

- <https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/cmdkey>
- <https://learn.microsoft.com/windows-server/security/windows-authentication/credentials-processes-in-windows-authentication#credential-storage-and-validation>
- <https://learn.microsoft.com/windows/security/identity-protection/remote-credential-guard>
