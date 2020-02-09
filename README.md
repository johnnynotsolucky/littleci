# LittleCI

**v0.0.0**

## Running

```bash
littleci serve --config /path/to/littleci.json
```

## Config

```javascript
{
  "secret": "<secret key>",
  "site_url": "http://localhost:8000",
  "network_host": "0.0.0.0",
  "port": 8000,
  "authentication_type": "Simple"
}
```

## TODO

- [ ] Add gitea integration
- [ ] Make cross compilation actually work
- [ ] Auto-run DB migrations
- [ ] Create DB and DB folders on first run
- [ ] Create default user on first run

