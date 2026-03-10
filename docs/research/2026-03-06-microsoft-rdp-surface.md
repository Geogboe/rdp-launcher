# Microsoft RDP Surface Research

Date: 2026-03-06

## Goal

Capture the Microsoft-documented surfaces that matter for a Windows MSTSC launcher and connection manager.

## Findings

### MSTSC Is Still The Practical Launch Surface

- Microsoft documents `mstsc.exe` as the inbox Remote Desktop Connection app.
- `mstsc.exe` explicitly supports launching with a `.rdp` file and editing an existing `.rdp` file.
- The command-line surface is intentionally small, which reinforces using generated `.rdp` files rather than trying to squeeze behavior through flags alone.
- `mstsc.exe /l` enumerates local monitor IDs, which is useful if the product supports `selectedmonitors`.

Source:

- <https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/mstsc>

### The `.rdp` Property Surface Is Large And Suitable As The Product Schema

- Microsoft publishes a shared catalog of supported RDP properties and explicitly says that Remote PC connections use `.rdp` files as the configuration point.
- The documented property set covers connections, session behavior, device redirection, display settings, and RemoteApp-related options.
- The `full address` property is documented as the only mandatory property for a `.rdp` file.
- Some properties have caveats, deprecations, or product-specific applicability. For example, `desktopscalefactor` is marked as being deprecated.

Source:

- <https://learn.microsoft.com/en-us/azure/virtual-desktop/rdp-properties>

### Windows Credential Manager Is A Supported Credential Surface

- Microsoft documents `cmdkey` for creating, listing, and deleting stored credentials.
- Windows documents Credential Manager as an encrypted vault used by apps that integrate through the Credential Manager APIs.
- This supports a design where the launcher can interoperate with Windows credential storage when needed, but it does not require the launcher to become a vault itself.

Sources:

- <https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/cmdkey>
- <https://learn.microsoft.com/windows-server/security/windows-authentication/credentials-processes-in-windows-authentication#credential-storage-and-validation>

### Secure Launch Modes Matter

- Microsoft documents both `/restrictedAdmin` and `/remoteGuard` on `mstsc.exe`.
- Remote Credential Guard is positioned as protecting credentials by keeping them off the remote host and redirecting Kerberos back to the client.
- Remote Credential Guard has environmental constraints, notably Kerberos and direct connection requirements.

Sources:

- <https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/mstsc>
- <https://learn.microsoft.com/windows/security/identity-protection/remote-credential-guard>

### MSTSC Is A Better Baseline Than RDCMan For This Product Direction

- Microsoft has published guidance recommending Windows built-in Remote Desktop Connection or the universal client instead of RDCMan.
- That does not mean MSTSC is a great management UX on its own; it means a launcher built around MSTSC is aligned with Microsoft's supported baseline.

Source:

- <https://learn.microsoft.com/troubleshoot/windows-client/remote/use-mstsc-universal-remote-desktop-client-instead-rdman>

## Product Implications

- The product should treat `.rdp` properties as the canonical configuration surface.
- The product should generate `.rdp` files at launch time rather than relying on MSTSC flags for most settings.
- Session tracking should remain local and process-aware rather than pretending the product has deep protocol insight.
- Security posture should surface launch modes such as prompt, saved credentials, Restricted Admin, and Remote Credential Guard with clear warnings and requirements.

## Follow-Up Research

- Confirm whether any important Remote PC properties used by MSTSC are absent from the published property catalog.
- Confirm the exact target naming conventions MSTSC uses with Windows Credential Manager so temporary credential cleanup can be precise.
- Confirm what local signals are available for distinguishing launched, active, disconnected, and failed MSTSC sessions.
