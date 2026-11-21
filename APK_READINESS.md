# Personal OS — Android APK Readiness Report

Target packaging route: **PWABuilder → Trusted Web Activity (TWA)**, all free.
Public app URL: **https://povgdh.duckdns.org**

> Do NOT generate the APK yet. This report is the pre-flight. Generate only when you
> (the owner) say go, after the final build is deployed.

## 1. PWA validation status — PASS (verified in a real browser)

| Check | Status |
|---|---|
| Web App Manifest present & valid JSON | ✅ `/manifest.webmanifest` |
| `name`, `short_name`, `description` | ✅ Personal OS / POS / set |
| `start_url`, `scope`, `id` | ✅ `/`, `/`, `/` |
| `display` | ✅ `standalone` (TWA-compatible) |
| `orientation`, `lang`, `dir`, `categories` | ✅ any / en / ltr / productivity, lifestyle |
| `theme_color`, `background_color` | ✅ `#0E0F12` (splash) |
| Icons 192 + 512 | ✅ present, correct dimensions |
| Maskable icon | ✅ declared (see blocker #3 for a polish note) |
| Screenshots (wide + narrow) | ✅ `1280x800` + `390x844` |
| Service worker registered & active | ✅ scope `/`, network-first |
| **Offline**: shell served from cache | ✅ offline reload → 200, app renders |
| Mobile viewport + touch | ✅ `viewport-fit=cover`, tap highlights off |
| HTTPS | ✅ served by your Caddy on the DuckDNS domain |
| Apple/iOS install tags | ✅ apple-touch-icon + web-app-capable added |

## 2. Required assets — DONE (ship in the build)
- `web/icon-192.png`, `web/icon-512.png`
- `web/screenshot-wide.png` (1280×800), `web/screenshot-narrow.png` (390×844)
- `web/manifest.webmanifest` (updated)

## 3. Packaging steps (when authorized)
1. Deploy the **final build** so production serves the updated manifest + screenshots
   (see `DEPLOY_CHECKLIST.md`). Confirm `https://povgdh.duckdns.org/manifest.webmanifest`
   shows the new fields.
2. Go to **https://www.pwabuilder.com**, enter `https://povgdh.duckdns.org`, Start.
3. Review the scorecard (should be green on manifest + service worker).
4. **Package For Stores → Android → Generate**. Options: package id e.g.
   `org.duckdns.povgdh.personalos`, app name "Personal OS", keep "signing key: new"
   (PWABuilder generates one) — **download and keep the signing key + key info safe**;
   losing it means you can't ship updates under the same app.
5. PWABuilder returns a zip with the `.aab`/`.apk`, the signing key, and
   `assetlinks.json` plus instructions.

## 4. Critical post-package step — Digital Asset Links
For the TWA to run full-screen (no browser URL bar), the site must prove it owns the app:
- Host PWABuilder's `assetlinks.json` at
  **`https://povgdh.duckdns.org/.well-known/assetlinks.json`** (served over HTTPS).
  - Easiest with your setup: put the file in `web/.well-known/assetlinks.json` — the API's
    `ServeDir` will serve it at that path. (I can wire this into the build once PWABuilder
    gives you the file; it contains the app's SHA-256 signing fingerprint.)
- Verify: `curl https://povgdh.duckdns.org/.well-known/assetlinks.json` returns the JSON.
- Without this, the app still installs but shows a URL bar.

## 5. Outstanding blockers / notes
1. **Deploy the final build first** — production must serve the new manifest before PWABuilder
   reads it. (Blocker until the next deploy.)
2. **assetlinks.json** — can only be created after PWABuilder generates the signing key
   (step 4 → 5). Not a code blocker; a one-file host step I'll help wire in.
3. **Maskable icon polish (optional)** — the 512 icon is declared maskable but wasn't drawn with
   Android's safe-zone padding, so the adaptive icon may be cropped slightly. For a crisp icon,
   regenerate a 512 with ~20% padding (PWABuilder's image generator does this free). Not blocking.
4. **Installability** otherwise fully met; no code changes needed to package.

## 6. What this does NOT need (confirmed free / no paid services)
- No Play Store account required to produce the APK/AAB (PWABuilder is free; a Play Store
  listing later is a separate, optional $25 one-time Google fee — not needed to install the APK
  directly).
- No paid packaging service, no framework migration.
