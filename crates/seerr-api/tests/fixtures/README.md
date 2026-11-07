# Seerr fixtures

Slim, redacted fixtures based on the read-only capture against Jellyseerr 3.5.1 on 2026-11-05.
They keep the fields the client reads plus a few unknown ones, so parsing stays permissive. No
credentials, user email, hostname, `serviceUrl`, `mediaUrl` or `jellyfinMediaId` from the live
instance is committed.

`requests-downloading.json` is **synthetic**: the live instance had only completed season-pack
rows at capture, so this file supplies the mid-progress movie and the two-download series used by
the percentage/aggregate tests. The live-shaped `requests.json` is the no-download and completed
case.
