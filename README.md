# LittleCI

**v0.0.0**

## Setup

```bash
./target/release/littleci serve --config /path/to/littleci.json
```

## Running

```bash
littleci serve --config /path/to/littleci.json
```

## Config

```javascript
{
  "secret": "<secret key>",
  "network_host": "0.0.0.0",
  "port": 8000,
  "authentication_type": "Simple"
  "data_dir": "/path/to/littleci/data"
}
```

## TODO

- [ ] Add gitea integration
- [ ] Make cross compilation actually work
- [ ] Auto-run DB migrations
- [ ] Create DB and DB folders on first run
- [ ] Create default user on first run

