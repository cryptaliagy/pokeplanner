# Install

PokePlanner uses [Nix](https://nixos.org/) with [direnv](https://direnv.net/) to provide a reproducible development environment. Once set up, all dependencies (Rust toolchain, protobuf, buf, openssl) are managed automatically.

## Prerequisites

### 1. Install Nix

Install Nix with flake support enabled:

```bash
curl -L https://install.determinate.systems/nix | sh -s -- install
```

This installer from [Determinate Systems](https://github.com/DeterminateSystems/nix-installer) enables flakes and the unified CLI by default.

Alternatively, use the official installer and enable flakes manually:

```bash
sh <(curl -L https://nixos.org/nix/install) --daemon
```

Then add to `~/.config/nix/nix.conf`:

```
experimental-features = nix-command flakes
```

### 2. Install direnv

Install direnv via your system package manager or Nix:

```bash
# Via Nix (recommended if you already have Nix)
nix profile install nixpkgs#direnv

# Or via system package manager
# Ubuntu/Debian: sudo apt install direnv
# macOS:         brew install direnv
```

### 3. Hook direnv into your shell

Add the appropriate line to your shell config:

```bash
# zsh (~/.zshrc)
eval "$(direnv hook zsh)"

# bash (~/.bashrc)
eval "$(direnv hook bash)"

# fish (~/.config/fish/config.fish)
direnv hook fish | source
```

Restart your shell or source the config file after adding the hook.

## Getting started

1. Clone the repository and `cd` into it:

   ```bash
   git clone <repo-url> pokeplanner
   cd pokeplanner
   ```

2. Allow direnv to load the environment:

   ```bash
   direnv allow
   ```

   The first run will download and build the Nix dependencies (Rust toolchain, protobuf, buf, openssl). Subsequent loads are instant.

3. Verify the setup:

   ```bash
   rustc --version
   cargo --version
   protoc --version
   buf --version
   ```

4. Build and test:

   ```bash
   cargo build
   cargo test
   ```

## Without direnv

If you prefer not to use direnv, enter the dev shell manually:

```bash
nix develop
```

This drops you into a shell with all dependencies available. You need to run this each time you open a new terminal.

## What the dev shell provides

| Tool       | Purpose                          |
|------------|----------------------------------|
| Rust (stable, latest) | Compiler, cargo, rust-analyzer, rust-src |
| protobuf   | `protoc` compiler for gRPC       |
| buf        | Proto file management            |
| pkg-config | Native dependency resolution     |
| openssl    | TLS support                      |
