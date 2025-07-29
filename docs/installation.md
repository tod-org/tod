# Installation

<!--toc:start-->
- [Installation](#installation)
  - [Homebrew](#homebrew-linux-mac-or-wsl)
  - [Crates.io](#cratesio--cargo-all-platforms)
  - [Scoop](#scoop-windows)
  - [GitHub](#manually-build-from-github-linux-mac-and-windows)
<!--toc:end-->

## [Homebrew](https://brew.sh) (Linux, Mac, or WSL)

```bash
brew tap tod-org/tod
brew install tod
```

## [Crates.io / Cargo](https://crates.io/crates/tod) (All Platforms)

[Install Rust](https://www.rust-lang.org/tools/install)

```bash
# Linux and MacOS
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Install [Tod](https://crates.io/crates/tod)

```bash
cargo install tod
```

## [Scoop](https://scoop.sh/) (Windows)

```powershell
scoop bucket add tod https://github.com/tod-org/tod
scoop install tod
```

## Manually Build from GitHub (Linux, Mac, and Windows)

[Install Rust](https://www.rust-lang.org/tools/install)

Clone the project

```bash
git clone git@github.com:tod-org/tod.git
cd tod
```

Test and build the release

```bash
./test.sh # run the tests
cargo build --release
```

You can then find the binary in `/target/release/`

It will ask you to OAuth or provide your [Todoist API token](https://todoist.com/prefs/integrations) on the first run
