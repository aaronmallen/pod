# Pod

A one-stop-shop capsuleer manager for [EVE Online](https://www.eveonline.com/), built as a pure-native Rust desktop
application.

## What it does

Pod aims to be the daily-driver companion app for capsuleers — everything you'd otherwise juggle between EVEMon, Pyfa,
and the in-game client, in one place:

- **Skill queue management** — view, plan, and manage your training queue across multiple characters.
- **Skill plans** — build, save, and share long-term training plans (think EVEMon-style).
- **In-game mail** — read, search, and respond to EVE mail without logging in.
- More to come as the project matures.

All character data is pulled live from the official [EVE Swagger Interface (ESI)](https://esi.evetech.net/) via OAuth2
SSO.

## Development

Item icons (~19,740 64px PNGs) are **not** committed to the repository — they are generated fresh during the release
pipeline and shipped inside each package. A fresh checkout therefore has no item icons: the app falls back to
silhouettes, and the best-effort sync long-tail fills them in over time.

If you want the real icons locally, generate them once (downloads the SDE and fetches each icon from the EVE image
server):

```sh
mise run generate:item-images
```

The output lands in `assets/images/items/` and is gitignored. See [docs/dev](docs/dev) for the full development guide.

## Support development

Pod is free and MIT-licensed. If you want to back development, the easiest way is in-game — send ISK to the **Pod
Developers** corporation (`PODEV`). Every donation goes straight back into time on this binary.

| Field  | Value           |
|--------|-----------------|
| Name   | Pod Developers  |
| Ticker | PODEV           |
| CEO    | Pod Dev         |

In-game: open your wallet, choose **Give ISK**, and paste **Pod Developers** as the recipient.

No subscriptions, no ads. Telemetry is anonymous, opt-out, and never tied to you.

## Attribution

EVE Online and the EVE logo are the registered trademarks of Fenris Creations (formerly CCP hf.). All rights reserved
worldwide. All other trademarks are the property of their respective owners. EVE Online, the EVE logo, EVE and all
associated logos and designs are the intellectual property of Fenris Creations. All artwork, screenshots, characters,
vehicles, storylines, world facts or other recognizable features of the intellectual property relating to these
trademarks are likewise the intellectual property of Fenris Creations. Fenris Creations has granted permission to Pod
to use EVE Online and all associated logos and designs for promotional and information purposes but does not endorse,
and is not in any way affiliated with, Pod.
