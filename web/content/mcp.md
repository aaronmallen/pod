---
title: MCP Server
section: Reference
order: 3
description: Pod ships an off-by-default Model Context Protocol server so AI agents like Claude Desktop and Claude Code can read and automate Pod over localhost, behind a bearer token and a per-tool permission model.
---

# MCP Server

Pod can run an embedded Model Context Protocol (MCP) server so AI agents, such
as Claude Desktop or Claude Code, can read your data and automate Pod on your
behalf. The server stays off until you turn it on, binds to localhost only, and
exposes nothing without your token.

Configure it under Settings, in the MCP Server tab.

![The MCP Server settings tab with the master switch, port, token, and tool permissions](/docs/img/mcp/settings-mcp.png)

## Turning it on

The Server section holds the master switch. Everything below it is inert until
the server is running, and the tab badge reads "Off" while it is disabled. Flip
the switch on and the badge shows the port the server is listening on, such as
":7373".

The server binds to 127.0.0.1, localhost only, and that address is fixed. It is
never reachable from another machine on your network. Only the port is
configurable, with a default of 7373. Type a new port and Pod keeps it in range.

## The bearer token

Agents authenticate with a bearer token. Every request must present it, so treat
it like a password. Pod generates a token automatically the first time you
enable the server.

The Authentication section shows the token masked. Use "Show" to reveal it and
"Copy token" to put it on your clipboard for pasting into an agent's config.
"Reset token" generates a new token and immediately invalidates the old one. Any
agent that was connected stops working until you re-paste the new token into its
config.

## Tool permissions

The Permissions section sets what a connected agent is allowed to do. A disabled
tool returns a permission-denied error to the agent. The permissions split into
local tools and tools that reach EVE.

Two local tools are on by default:

- **Read tools** let agents see your characters, wallet, ledger, market,
  contracts, skills, industry and planner, assets, mail, and prices. Read-only:
  nothing is written and nothing leaves Pod.
- **Local write tools** let agents assign budgets, build skill plans, and
  configure the industry planner. These write only to Pod's local database, so
  there is no EVE-side effect.

Three tools reach EVE and are off by default, so the actions that touch the game
are an explicit opt-in:

- **Send mail** lets agents compose and send EVE mail from your characters.
- **Delete mail** lets agents permanently delete messages from your EVE mail.
- **Manage labels** lets agents create, rename, delete, and reassign your mail
  labels.

The permission badge reads "Local only" when no EVE-reaching tool is on, and
counts the EVE effects you have enabled otherwise.

## Connecting an agent

The Connect an agent section gives you a ready-made config block for your agent,
for example claude_desktop_config.json. "Copy config" copies it with your real
bearer token filled in, so you paste it in and go. After you add Pod to the
agent's MCP config, restart the agent and it discovers Pod's tools
automatically.

Everything an agent does is recorded to Pod's normal logs. Review them under
Settings, in the Storage tab.
