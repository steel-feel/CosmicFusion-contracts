# CosmicFusion-contracts

Destination escrow contracts for 1inch cross chain swap

## Usage

### Build contract

```bash
cargo wasm
```

**Optimised version for testnets**
```bash
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/optimizer:0.16.0
```

### Testing
```bash
cargo test
```

#### show print output
```bash
cargo test -- --show-output
```

#### Check the wasm file 
```bash
cosmwasm-check <wasm-file>
```

### Generate schema (required for ts bindings)
```bash
cargo schema
```


