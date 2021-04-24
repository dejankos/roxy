# Roxy - reverse  proxy (WIP)
Reverse proxy with support for live configuration updates, balancing strategies, ssl and caching.

## Usage
Run service with path to configuration file or defaults will be used.

```
USAGE:
roxy [OPTIONS]

FLAGS:
-h, --help       Prints help information
-V, --version    Prints version information

OPTIONS:
-p, --proxy-config-path <proxy-config-path>    Proxy configuration file [default: config/proxy.yaml]
```

## Proxy Configuration
```
# service configuration
service:
  # ip address
  ip: localhost
  # bind port
  port: 8080
  # worker threads
  workers: 6
  # dev mode - will enable only terminal logger
  dev_mode: true

# inbound paths
inbound:
  # match path to group
  - path: /abc/*
    group: group_1
  # match path to group
  - path: /cde/*
    group: group_2

# outbound server groups
outbound:
  - group: group_1
    # timeout for all servers in this group
    timeout: 60
    # backend servers for this group
    # round robin balancing to all servers
    servers:
      - http://localhost:8080/push
  - group: group_2
    servers:
      - http://test:8082
      - http://test2:8181/test
```

## Build from source
### Install Rust
```bash
$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
Update to latest stable version
```bash
$ rustup update
```
### Install openssl
```
# macOS (Homebrew)
$ brew install openssl@1.1

# macOS (MacPorts)
$ sudo port install openssl

# macOS (pkgsrc)
$ sudo pkgin install openssl

# Arch Linux
$ sudo pacman -S pkg-config openssl

# Debian and Ubuntu
$ sudo apt-get install pkg-config libssl-dev

# Fedora
$ sudo dnf install pkg-config openssl-devel
```
### Build
```bash
$ cargo build
```

## Licence
Rocky is licensed under the [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
