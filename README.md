# Roxy - reverse  proxy (WIP)
Reverse proxy with support for live configuration updates, balancing strategies, ssl and  we'll see what else.

## Usage

## Configuration

```
inbound:
  - path: /abc/*
    group: group_1
  - path: /cde/*
    group: group_2

outbound:
  - group: group_1
    timeout: 60
    servers:
      - http://someurl:port/path
  - group: group_2
    servers:
      - http://someurl:port/path
      - https://someurl2:port/path2