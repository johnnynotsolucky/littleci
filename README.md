# LittleCI

WIP

**v0.0.0**

## Requirements

LittleCI uses the [Rocket](https://github.com/SergioBenitez/Rocket) web
framework and therefore requires Rust nightly.

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

## Running

When launched without the `--config` flag, LittleCI will check the working
directory for a `littleci.json` configuration file, if it's not found, one will
be created with default configuration.

```bash
littleci serve --config /path/to/littleci.json
```

## License

This project is licensed under [the Parity License](LICENSE-PARITY.md).
Third-party contributions are licensed under either [MIT](LICENSE-MIT.md) or
[Apache-2.0](LICENSE-APACHE.md) and belong to their respective authors.

