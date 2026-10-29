# Demo Tool

Demo Tool turns notes into searchable answers for small teams.

It indexes your folders locally and answers questions from them.

## Features

- Ask questions about your documents

## Requirements

- Windows 10 or later

## Installation

```bash
curl -fsSL https://example.com/install.sh | sh
demo --version
```

## Quick Start

1. Run `demo add ./notes`.
2. Ask `demo "what changed last week?"`.

## Usage

```bash
demo sync
```

## Configuration

| Setting | Default |
|---|---|
| `DEMO_HOME` | `~/.demo` |

## Troubleshooting

Logs are in `~/.demo/logs`.

## Documentation

- [Technical documentation](docs/TECH.md)

## License

MIT - see [LICENSE](LICENSE).
