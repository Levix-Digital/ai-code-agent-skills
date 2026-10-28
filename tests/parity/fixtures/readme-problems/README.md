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

### Prompt history

Use the arrow keys.

### Updating

Run `demo update`.


```bash
demo sync
```

## Configuration

| Setting | Default |
|---|---|
| `DEMO_HOME` | `~/.demo` |

## Solução de Problemas

Logs are in `~/.demo/logs`.

## Documentation

- [Technical documentation](docs/TECH.md)

## License

MIT - see [LICENSE](LICENSE).

## What's New

- Faster sync

### Hybrid search (`v0.32.4`)

Details.

## Changelog

## Release 2024-05-01

See ADR-0130 and [the engine](src/engine.rs). Added in v0.45.0.

```mermaid
flowchart LR
 A-->B
```

Este parágrafo não está em inglês.

Requires Python 3.10.0 or later. <!-- readme-ok -->

```bash
pip install demo==1.2.3
```

See [missing](docs/NOPE.md), [here](#no-such-section), [ok](#installation), [web](https://example.com/a.py) and `[code](nope.md)`.

# Second Title
