# ADR 0001: Use MSTSC As The Execution Engine

## Status

Accepted

## Context

The product goal is to improve the experience of launching and managing RDP connections on Windows without taking on the cost and risk of implementing an RDP client. Microsoft still documents and supports `mstsc.exe` as the inbox Remote Desktop Connection client and documents `.rdp` file launch and edit flows.

The alternative directions were:

- build a custom RDP client
- target the Windows App or older Store client as the main execution engine
- wrap a heavier management tool such as RDCMan

## Decision

Use `mstsc.exe` as the execution engine. The application will generate temporary `.rdp` files for launches and use MSTSC for session startup.

## Consequences

- The product remains aligned with supported Windows behavior.
- The `.rdp` property model becomes a core design concern.
- The app can focus on UX, planning, visibility, and automation rather than protocol implementation.
- Some behavior remains constrained by MSTSC and Windows policy.
- Credential handling must respect MSTSC expectations rather than inventing its own connection protocol.

## References

- <https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/mstsc>
- <https://learn.microsoft.com/en-us/azure/virtual-desktop/rdp-properties>
- <https://learn.microsoft.com/troubleshoot/windows-client/remote/use-mstsc-universal-remote-desktop-client-instead-rdman>
