# Iris

Client mail de bureau (IMAP/SMTP) en Tauri 2 + Rust + Svelte 5, pour Windows et Linux. Iris est la suite de Courrier.

## Config
Les réglages de connexion sont saisis dans l'application ; le mot de passe est conservé dans le trousseau système (service `dev.dksuite.iris`).
Les réglages de Courrier (`<config>/courrier/settings.json`) sont repris automatiquement au premier lancement.

Mode dev sans écran de connexion : `IRIS_ENV=/chemin/.env` avec
```
MAIL_USER=...
MAIL_PASS=...   # mot de passe d'application
IMAP_HOST=imap.fastmail.com
SMTP_HOST=smtp.fastmail.com
```

## Dev
`npm install && npx tauri dev`

## Tests
- `cd src-tauri && cargo test` (hors ligne ; les tests IMAP/SMTP en direct sont `#[ignore]`, à lancer avec `cargo test -- --ignored`)
- `npx vitest run`, `npx svelte-check`

## Release
`git tag v0.1.0 && git push --tags` : la GitHub Action construit et publie les installeurs.

## Mises à jour
Tauri updater : l'app lit `releases/latest/download/latest.json` (signé par la clé `~/.tauri/iris.key`, secret `TAURI_SIGNING_PRIVATE_KEY`). Ne pas perdre cette clé.
