# Roxy - Reverse Proxy

## Overview
Roxy is a reverse proxy server written in Rust that provides load balancing, response caching, and live configuration updates without service restarts. The project is currently in a rewrite phase (WIP).

## What It Does
- Routes incoming HTTP requests to backend server groups based on path patterns
- Distributes traffic across multiple backend servers using round-robin load balancing
- Supports both HTTP and HTTPS backend servers with configurable SSL verification
- Caches HTTP responses with configurable TTL to reduce backend load
- Monitors configuration file changes and applies updates in real-time
- Adds X-Forwarded-For headers for request tracing

## Architecture

### Module Structure
The project uses a Rust workspace with three crates:

**server/**
- Entry point using Actix-web framework
- Binds to configured IP/port and spawns worker threads
- Delegates all requests to the proxy handler

**core/**
- `proxy.rs`: Main proxy logic, handles request forwarding and caching
- `balancer.rs`: Round-robin load balancing across server groups
- `config.rs`: Configuration management with live reload support
- `file_watcher.rs`: Monitors proxy.yaml for changes using notify crate
- `matcher.rs`: Regex-based path matching to route requests to server groups
- `http_utils.rs`: HTTP header manipulation utilities
- `log.rs`: Logging initialization

**cache/**
- `expiring_cache.rs`: Thread-safe response cache with TTL expiration
- Uses BlockingDelayQueue for efficient expiration management
- Capacity-limited to prevent memory exhaustion

### Data Flow
1. Request arrives at server (server/main.rs:57)
2. Matcher finds server group based on path (core/matcher.rs:37)
3. Balancer selects backend server via round-robin (core/balancer.rs:33)
4. Proxy checks cache, forwards request if miss (core/proxy.rs:66)
5. Response cached if cacheable (Cache-Control header present)
6. Response returned to client

### Key Design Patterns
- **Observer Pattern**: FileWatcher notifies Configuration on file changes
- **Strategy Pattern**: Pluggable balancing (currently only round-robin)
- **Sharded Locking**: Uses crossbeam ShardedLock for concurrent read access
- **Thread-Safe Caching**: Separate expiration thread prevents blocking

## Configuration
YAML-based configuration with four sections:
- `service`: Proxy server settings (IP, port, workers)
- `ssl` (optional): SSL/TLS configuration for HTTPS backends
- `inbound`: Path patterns mapped to server groups
- `outbound`: Backend server groups with timeouts

Example:
```yaml
service:
  ip: localhost
  port: 8080
  workers: 6

ssl:
  verify_ssl: true  # Set to false for self-signed certs

inbound:
  - path: /api/*
    group: api_servers

outbound:
  - group: api_servers
    timeout: 60
    servers:
      - http://backend1:8080
      - https://backend2:8443  # HTTPS backends supported
```

## Potential Improvements

### High Priority
1. ~~**Update Dependencies**~~: ✅ **COMPLETED** - Updated to Actix-web 4, notify 6, awc 3, clap 4, and all other dependencies
2. ~~**Enable HTTPS Support**~~: ✅ **COMPLETED** - Full HTTPS backend support with configurable SSL verification
3. **Health Checks**: Detect and remove unhealthy backends from rotation
4. **Better Error Handling**: Currently returns generic 500 errors
5. **Comprehensive Tests**: Minimal test coverage currently

### Medium Priority
6. **Additional Balancing Strategies**: Least connections, weighted round-robin, IP hash
7. **Circuit Breaker**: Prevent cascading failures from unhealthy backends
8. **Request Retry Logic**: Automatic retry on backend failures
9. **Metrics & Monitoring**: Prometheus/Grafana integration
10. **Rate Limiting**: Per-client or per-path rate limits

### Nice to Have
11. **Request/Response Transformation**: Header manipulation, path rewriting
12. **WebSocket Support**: Currently only HTTP
13. **Compression**: Gzip/Brotli support
14. **Access Logs**: Structured logging for requests
15. **Admin API**: Runtime configuration inspection without file edits
16. **Docker Support**: Containerization for easier deployment
17. **Graceful Shutdown**: Drain connections before stopping
18. **Connection Pooling**: Reuse backend connections for better performance

## Technical Debt
- Error messages could be more descriptive
- Configuration validation is minimal
- No integration tests for end-to-end flows
