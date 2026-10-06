# Wrybill: where it has been run

Wrybill is meant for any 64-bit laptop from 2015 onwards, on macOS, Windows and Linux ([SPEC.md](SPEC.md), sections 14.1 and 17). This page records the machines it has really been run on, and what happened there, so nobody has to take that on trust.

It's early days. So far "run" means `wrybill doctor` and `wrybill keys set`, which is what milestone M0 built. Later milestones will add model servers and browsers to this page.

## Real machines

| Machine | OS | Chip and memory | Build | Date | What was run | What happened |
|---|---|---|---|---|---|---|
| The maintainer's Mac | macOS 26.7.1 | Apple M5, 10 cores, 32 GB | `aarch64-apple-darwin`, commit `a12033c`, built on the machine | 6 October 2026 | `wrybill doctor`, and `wrybill keys set` with a made-up key | Both worked. Doctor took 0.3 seconds. It found a key that a different build had saved, and macOS showed no Keychain prompt. |

**Still to do: a real laptop from around 2015.** None has been tested yet. It's postponed, not dropped: it waits for a contributor. A 2015 MacBook on macOS 11 or 12, a Windows 10 laptop or an old ThinkPad on Linux would all do. The steps are [further down](#run-it-on-your-own-machine).

## GitHub's hosted runners

CI runs `wrybill doctor` on each of these for every pull request, and keeps what it prints. They're virtual machines in a data centre. They show that each build starts and reports sensibly on that OS, and nothing about how Wrybill feels on a real laptop.

The table is from the CI run for commit `a12033c` on 6 October 2026. The hardware behind a runner changes from run to run.

| Runner | Build | OS | Chip | What happened |
|---|---|---|---|---|
| `macos-15-intel` | `x86_64-apple-darwin` | macOS 15.7.9 | Intel Core i7-8700B, 4 cores | Worked, in 0.4 seconds |
| `macos-26` | `aarch64-apple-darwin` | macOS 26.6.2 | Apple M1 (Virtual), 3 cores | Worked, in 0.2 seconds. Graphics came back as "Unknown" |
| `windows-2025` | `x86_64-pc-windows-msvc` | Windows Server 2025 Datacenter, build 26100 | AMD EPYC 7763, 2 cores | Worked, in 0.2 seconds. Security updates are "Unknown", because Server editions aren't in Wrybill's table |
| `windows-11-arm` | `aarch64-pc-windows-msvc` | Windows 11 Enterprise, build 26200 | 4 cores. Windows gave no name for the chip | Worked, in 0.2 seconds |
| `ubuntu-24.04` | `x86_64-unknown-linux-gnu` | Ubuntu 24.04 | Intel Xeon Platinum 8573C, 2 cores | Worked, in 0.1 seconds |
| `ubuntu-24.04` | `x86_64-unknown-linux-musl` | Ubuntu 24.04 | AMD EPYC 9V74, 2 cores | Worked, in 0.1 seconds |
| `ubuntu-24.04-arm` | `aarch64-unknown-linux-gnu` | Ubuntu 24.04 | Neoverse-N2, 4 cores | Worked, in 0.1 seconds |
| `ubuntu-24.04-arm` | `aarch64-unknown-linux-musl` | Ubuntu 24.04 | Neoverse-N2, 4 cores | Worked, in 0.1 seconds |
| `ubuntu-22.04` | `x86_64-unknown-linux-musl` | Ubuntu 22.04 | AMD EPYC 7763, 2 cores | Worked, in 1.0 seconds. The static build, on an older system than it was built on |
| `ubuntu-22.04-arm` | `aarch64-unknown-linux-musl` | Ubuntu 22.04 | Neoverse-N2, 4 cores | Worked, in 1.0 seconds. The static build, on an older system than it was built on |
| QEMU on `ubuntu-24.04`, playing a `qemu64` chip and then a `Nehalem` chip | `x86_64-unknown-linux-musl` | Ubuntu 24.04 | Reported with no AVX and no AVX2, both times | Worked, in 0.1 seconds. This shows the build runs on a chip from before AVX, which no test laptop from 2015 can show |

On every runner doctor exited with 0, and its first line named the commit and the build.

**The keychain.** Doctor reached the macOS Keychain and Windows Credential Manager. On Linux it reached the Secret Service on the runners where CI installs GNOME Keyring. On the Ubuntu 22.04 and QEMU runs there's no keyring, and doctor said so and pointed to `env:` references instead. CI also saves and reads back a made-up key through the real keychain on all eight builds.

**What's installed.** On every runner doctor found the shells, git, Python, Node.js, Java and at least one browser, each with its version where it asks for one. It found a package manager (Homebrew, winget or apt) on all but the Windows ARM runner, which has none of the six it looks for. It found Docker on the Linux runners and on `windows-2025`, and reported it as not installed on the rest.

## Run it on your own machine

### 1. Get a build

Every pull request's CI run keeps a build for each kind of machine for 14 days. Open the run from the pull request's **Checks** tab, scroll down to **Artifacts**, and download the one for your machine. You need to be signed in to GitHub.

| Your machine | The build to download |
|---|---|
| A Mac with Apple silicon | `wrybill-aarch64-apple-darwin.tar.gz` |
| An Intel Mac | `wrybill-x86_64-apple-darwin.tar.gz` |
| Windows on an Intel or AMD chip | `wrybill-x86_64-pc-windows-msvc.tar.gz` |
| Windows on an ARM chip | `wrybill-aarch64-pc-windows-msvc.tar.gz` |
| Linux on an Intel or AMD chip | `wrybill-x86_64-unknown-linux-musl.tar.gz` |
| Linux on an ARM chip | `wrybill-aarch64-unknown-linux-musl.tar.gz` |

The two Linux builds listed here are static, so they run on any distribution. The `-gnu` builds beside them need a recent system, such as Ubuntu 24.04.

If you have Rust installed, you can build it yourself instead. In a copy of this repository, run `cargo run --release -- doctor`.

### 2. Unpack it and run it

These builds aren't signed yet. Signing comes with the first release (M8), so macOS and Windows may ask whether you trust the file.

**macOS**, in Terminal, in the folder you downloaded it to:

```sh
tar -xzf wrybill-aarch64-apple-darwin.tar.gz
./wrybill-aarch64-apple-darwin/wrybill doctor
```

If macOS refuses to open it because it can't check who made it, tell macOS that you trust this one file, and run it again:

```sh
xattr -d com.apple.quarantine wrybill-aarch64-apple-darwin/wrybill
```

**Windows 10 or 11**, in PowerShell, in the folder you downloaded it to:

```powershell
tar -xzf wrybill-x86_64-pc-windows-msvc.tar.gz
.\wrybill-x86_64-pc-windows-msvc\wrybill.exe doctor
```

If a blue "Windows protected your PC" box appears, choose **More info**, then **Run anyway**.

**Linux**, in a terminal, in the folder you downloaded it to:

```sh
tar -xzf wrybill-x86_64-unknown-linux-musl.tar.gz
./wrybill-x86_64-unknown-linux-musl/wrybill doctor
```

Use the file name of the build you downloaded in place of the one shown.

### 3. Send in what it prints

Open an issue with your machine's make, model and year in the title, and paste in everything `wrybill doctor` printed.

It's safe to post. Doctor leaves out your computer's name, your user name, serial numbers, network addresses and paths, and it never shows a key.

Tell us too about anything it got wrong: a browser or a program it missed, the wrong chip, or an "Unknown" where you'd expect an answer. That's exactly what this page is for.
