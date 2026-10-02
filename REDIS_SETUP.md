# Redis Setup

The Cross-Node Signaling architecture requires a running Redis instance to bridge multiple Axum gateways.

During environment detection, it was determined that:
1. **Docker** is not installed or not available in the PATH.
2. **WSL2** (Windows Subsystem for Linux) is disabled because "Virtual Machine Platform" is not enabled in the BIOS/Windows Features.

Since neither containerization nor Linux virtualization is available, you have two options to run Redis locally on Windows:

## Option 1: Native Windows Port (Community)
You can download a native Windows port of Redis (e.g. from [Memurai](https://www.memurai.com/) or the older Microsoft Open Tech archive).
1. Download and extract the binaries.
2. Run `redis-server.exe`.
3. The server will bind to `127.0.0.1:6379`.

## Option 2: Enable WSL2
Enable WSL2 to run the official Linux Redis binaries:
1. Open PowerShell as Administrator and run:
   ```powershell
   wsl.exe --install
   ```
2. Enable "Virtual Machine Platform" in Windows Features.
3. Restart your computer.
4. Open the Ubuntu WSL terminal and run:
   ```bash
   sudo apt update
   sudo apt install redis-server
   sudo service redis-server start
   ```

Once Redis is running, the Rust cross-node integration tests (`cargo test --test integration_tests`) will successfully pass messages between Gateway A and Gateway B.
