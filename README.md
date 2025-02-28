# <h1 align="center"> An Orderbook AVS on EigenLayer using Tangle SDK </h1>

**An order book AVS for EigenLayer with the BLS-based Contract Configuration**

## 📚 Overview

This project is about creating an orderbook AVS for EigenLayer using the Tangle SDK.
An AVS (Actively Validated Service) is an off-chain service that runs arbitrary computations for a user-specified period of time.

## 📚 Prerequisites

Before you can run this project, you will need to have the following software installed on your machine:

- [Rust](https://www.rust-lang.org/tools/install)
- [Forge](https://getfoundry.sh)
- [Anvil]
- [Docker]

You will also need to install [cargo-tangle](https://crates.io/crates/cargo-tangle), the CLI tool for creating and
deploying Blueprints:

To install the Tangle CLI, run the following command:

> Supported on Linux, MacOS, and Windows (WSL2)

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/tangle-network/gadget/releases/download/cargo-tangle-v0.1.2/cargo-tangle-installer.sh | sh
```

Or, if you prefer to install the CLI from crates.io:

```bash
cargo install cargo-tangle --force # to get the latest version.
```

To install forge and anvil, you will first need to install `foundryup` using this command:

```bash
curl -L https://foundry.paradigm.xyz | bash
```

Then, running `foundryup` will automatically install the latest stable version of the precompiled binaries: forge, cast, anvil, and chisel.

## 🚀 Getting Started

Once `cargo-tangle`, `rust`, `docker` and `forge` are installed, you will need to run the following commands:

Install the foundry dependencies with:

```sh
forge soldeer update -d
```

Build the smart contracts:

```sh
forge build
```

Build the release version of rust project:

```sh
cargo build --release
```

## Deploying the AVS

To deploy the AVS on your local testnet, you will need to run the command:

```sh
cargo tangle blueprint deploy eigenlayer \
  --devnet \
  --ordered-deployment
```

Make sure you deploy the TaskManager contract first before the ServiceManager. It will prompt you with addresses that you need to input. You will find most of them here:

| Contract Name        | Address                                      |
| -------------------- | -------------------------------------------- |
| Registry Coordinator | 0xc3e53f4d16ae77db1c982e75a937b9f60fe63690   |
| Pauser Registry      | Obtained from beginning of Deployment output |
| Initial Owner        | 0x70997970C51812dc3A010C7d01b50e0d17dc79C8   |
| Aggregator           | 0xa0Ee7A142d267C1f36714E4a8F75612F20a79720   |
| Generator            | 0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65   |
| AVS Directory        | 0x0000000000000000000000000000000000000000   |
| Rewards Coordinator  | 0x0000000000000000000000000000000000000000   |
| Stake Registry       | 0x5fc8d32690cc91d4c39d9d3abcbd16989f875707   |
| Tangle Task Manager  | Obtained in Deployment output                |

For more information on deploying this AVS, visit [the Tangle docs](https://docs.tangle.tools/developers/eigenlayer-avs/bls-template).

## Running the AVS

To run the AVS, you need to run the following command:

```sh
cargo tangle blueprint run \
  -p eigenlayer \
  -u <YOUR_RPC_URL> \
  --keystore-path ./test-keystore
```

Replace the RPC URL with the HTTP RPC endpoint URL obtained when you deployed the AVS. It's usually `http://localhost:55000`.

### Note:

Make sure that you have the `TASK_MANAGER_ADDRESS` defined in your environment file. Alternatively, you can set the variable in the command like so:

```sh
TASK_MANAGER_ADDRESS=<TASK_MANAGER_ADDRESS> cargo tangle blueprint run \
  -p eigenlayer \
  -u <YOUR_RPC_URL> \
  --keystore-path ./test-keystore
```

## Testing the AVS

Instead of deploying and running the AVS over and over again after each modification, you can use the rust integration tests in `test.rs` to test out the AVS.

To do this, simply run:

```sh
cargo test
```

This will deploy the AVS, create an operator, and spawn tasks to test the logic of the operator, aggregator and smart contracts.
