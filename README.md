# cashyyc

cashyyc(Container Access Shell Helper Yielding Your Container) is a DevContainer CLI.

cashyyc provides a simple way to enter a dev container from the terminal.

## Prerequisites

- Docker

## Installation

```bash
curl -fsSL https://raw.githubusercontent.com/toms74209200/cashyyc/master/install.sh | bash
```

The binary is installed to `~/.local/bin/cyyc`. Make sure `~/.local/bin` is in your `PATH`.

## Usage

Open a shell in the dev container. The container keeps running after you exit the shell:

```bash
cyyc shell
```

Stop the running container without removing it:

```bash
cyyc stop
```

### Multiple environments

If multiple devcontainer configs exist under `.devcontainer/`, specify the environment name:

```
.devcontainer/
├── python/
│   └── devcontainer.json
└── rust/
    └── devcontainer.json
```

```bash
cyyc shell python
```

```bash
cyyc stop python
```

## License

[MIT License](LICENSE)

## Author

[toms74209200](https://github.com/toms74209200)
