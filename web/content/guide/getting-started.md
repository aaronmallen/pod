---
title: Getting Started
section: Guide
order: 1
---

# Getting Started

Welcome to **Pod**, the free, native EVE Online companion. This page walks you
through installing Pod and adding your first character.

![Pod splash screen](/docs/img/getting-started/splash.png)

## Install Pod

1. Download the build for your platform from the [download page](/#download).
2. Open the installer and follow the prompts.
3. Launch Pod — you'll land on an empty roster ready for your first character.

On macOS you may need to allow the app in _System Settings → Privacy &
Security_ the first time you open it.

## Add your first character

Pod reads your character data through the public ESI with your explicit consent.
Adding a character takes a single round-trip through EVE's SSO:

- Click **Add a character** on the empty roster.
- Choose **Authorize with EVE SSO** and sign in.
- Pod catches the hand-off and your character card appears.

![Add a character panel](/docs/img/getting-started/auth-panel.png)

## Where your data lives

Everything Pod knows stays on your machine. The local database path is shown in
`Settings → Storage`, and you can point it anywhere you like:

```toml
[storage]
database = "~/Library/Application Support/pod/pod.db"
```

That's it — you're ready to explore the rest of the wiki.
