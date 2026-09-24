# Contributing to Senbit 🦀

Thank you for your interest in contributing to Senbit! 🚀

Senbit is an open-source, server-first operating system built around the Linux kernel. The project is developed openly, and contributions of all kinds are welcome — code, documentation, testing, bug reports, ideas, security research, and architecture discussions.

---

## 🧭 Before contributing

Senbit is currently in early development, so its architecture and development tools are still evolving.

For significant changes, new features, or architectural changes, please open a GitHub Discussion or Issue before starting implementation.

This allows the idea and its design to be discussed before substantial work is done.

Small bug fixes, documentation improvements, tests, and clearly scoped changes can generally be submitted directly as a Pull Request.

---

## 🚀 Getting started

Clone the repository:

```bash
git clone https://github.com/yo-le-zz/senbit.git
cd senbit
````

Create a branch for your changes:

```bash
git checkout -b feature/my-feature
```

Use a descriptive branch name.

Examples:

```text
feature/package-manager
feature/network-configuration
fix/kernel-build
fix/update-rollback
docs/contributing
docs/architecture
kernel/networking
build/improve-build-system
```

Avoid working directly on `main`.

---

## 🏗️ Project structure

Senbit is organized into several major areas.

The structure will evolve as the project develops, but the intended architecture currently includes:

```text
senbit/
├── kernel/        # Linux kernel configuration and patches
├── system/        # Core Senbit userspace
├── packages/      # Package and software integration
├── rootfs/        # Root filesystem
├── installer/     # Installation and deployment
├── tools/         # Development and administration tools
├── tests/         # Automated and manual tests
├── docs/          # Technical documentation
├── scripts/       # Build and development scripts
├── assets/        # Project assets and branding
└── .github/       # GitHub configuration
```

Do not create new directories simply to match this structure if they are not needed yet.

Keep the repository structure understandable and purposeful.

---

## 🦀 Development philosophy

Senbit uses the Linux kernel as its foundation and builds its own infrastructure around it.

The Linux kernel remains primarily a C project.

Senbit's own system components will primarily use Rust.

Rust is an important part of Senbit's architecture because memory safety is particularly valuable for system software and privileged components.

However, the language itself is not the objective.

Use the appropriate technology for the problem being solved.

### General principles

* Keep implementations simple.
* Prefer maintainable code over clever code.
* Avoid unnecessary dependencies.
* Avoid unnecessary abstractions.
* Consider security implications for privileged code.
* Consider performance where it matters.
* Document important architectural decisions.
* Keep components modular.
* Do not duplicate functionality that Linux or existing Linux infrastructure already provides well without a concrete reason.

---

## 🐧 Linux kernel changes

Senbit is based on the upstream Linux kernel.

Kernel changes should therefore be treated differently from Senbit-specific userspace code.

Whenever possible:

* Prefer upstream-compatible solutions.
* Avoid unnecessary long-term patches.
* Keep Senbit-specific kernel patches small and clearly documented.
* Do not modify unrelated kernel code.
* Document why a Senbit-specific kernel change is necessary.

The Linux kernel retains its own license and contribution requirements.

---

## 📦 Software compatibility

Senbit aims to remain compatible with the existing Linux software ecosystem.

The project intends to use Debian packages and APT rather than creating a completely separate package ecosystem.

Contributions should therefore avoid unnecessarily breaking compatibility with existing Linux software.

If a proposed change could affect compatibility, explain the impact in the Pull Request.

---

## 🔄 Updates and system changes

Senbit is designed to support reliable system updates, snapshots, rollback, and both online and offline deployment.

Changes affecting these systems should consider:

* Failure during an update.
* Power loss.
* Corrupted or incomplete data.
* Recovery after a failed update.
* Compatibility between system versions.
* Existing user configuration.
* Rollback behavior.
* Security verification.

A system update should never be designed around the assumption that everything will always go perfectly.

---

## 🛡️ Security-sensitive code

Security is a core Senbit objective.

When modifying security-sensitive code, consider:

* Privilege boundaries.
* Input validation.
* File permissions.
* Authentication and authorization.
* Cryptographic verification.
* Filesystem access.
* Process isolation.
* Network exposure.
* Failure behavior.
* Recovery behavior.

Never commit passwords, API keys, private keys, tokens, credentials, or other secrets.

If you discover a security vulnerability, do not create a public Issue or Discussion.

See [`SECURITY.md`](SECURITY.md).

---

## 🧪 Testing

Test your changes as thoroughly as reasonably possible before opening a Pull Request.

Senbit will progressively introduce automated tests as the project develops.

For now, testing may include:

* Building the affected component.
* Running the relevant scripts.
* Booting Senbit in QEMU.
* Testing on supported hardware.
* Testing failure cases.
* Testing configuration changes.
* Verifying that unrelated components still work.

When a change cannot currently be tested automatically, explain how you tested it manually.

---

## 🛠️ Build system

Senbit provides development scripts intended to make building and cleaning the project straightforward.

The planned interface includes:

```bash
./install.sh
```

Installs the tools required to build Senbit.

```bash
./build.sh
```

Builds Senbit.

The build system will support rebuilding only selected components when possible.

```bash
./clean.sh
```

Cleans generated build artifacts.

Different cleaning levels and component-specific cleaning will be supported.

A `Makefile` will also provide a convenient interface for common development operations.

The exact commands and available options may change during early development.

Always prefer the current project documentation and `--help` output over outdated examples.

---

## 📝 Commits

Write clear and meaningful commit messages.

Good examples:

```text
Add initial root filesystem
Fix kernel build configuration
Add network configuration parser
Implement snapshot metadata
Update installation documentation
```

Avoid vague messages:

```text
fix
update
changes
stuff
test
```

Keep commits focused.

A commit should preferably represent one logical change.

Avoid mixing unrelated changes into the same commit.

---

## 🔀 Pull Requests

Before opening a Pull Request:

* [ ] The changes are focused on one purpose.
* [ ] I tested the changes where possible.
* [ ] I updated relevant documentation.
* [ ] I removed temporary or debugging code.
* [ ] I checked for unintended changes.
* [ ] I considered security implications.
* [ ] I considered compatibility implications.
* [ ] I did not include secrets or credentials.
* [ ] I checked relevant licensing requirements.

Your Pull Request should explain:

### What

What does this Pull Request change?

### Why

Why is this change necessary?

### How

How was the change implemented?

### Testing

How was it tested?

### Limitations

Are there known limitations, edge cases, or follow-up tasks?

---

## 💡 Feature proposals

Before implementing a significant feature, explain:

1. The problem it solves.
2. Why it belongs in Senbit.
3. How it could work.
4. Possible alternatives.
5. Potential drawbacks.
6. Security implications.
7. Performance implications.
8. Compatibility implications.
9. How it could be tested.

Not every proposed feature will necessarily be accepted.

A discussion before implementation can prevent significant work from being built around an unsuitable design.

---

## 🤖 AI-assisted development

Contributors may use AI-assisted development tools if they wish.

However, contributors remain fully responsible for the code they submit.

AI-generated code must be:

* Reviewed by the contributor.
* Understood by the contributor.
* Tested appropriately.
* Compatible with the project's architecture.
* Checked for security issues.
* Checked for unnecessary complexity.
* Checked for licensing or attribution requirements where applicable.

Submitting AI-generated code does not transfer responsibility to the AI tool or its provider.

Do not submit code simply because an AI tool generated it.

---

## 📚 Documentation

Documentation is part of the project.

When changing behavior, configuration, commands, architecture, or public interfaces, update the relevant documentation when necessary.

Documentation contributions are welcome even when they do not include code changes.

---

## 🌍 Community

Senbit is built in the open.

You can participate through:

* 💡 GitHub Discussions
* 🐛 GitHub Issues
* 🔀 Pull Requests
* 📚 Documentation
* 🧪 Testing
* 🔐 Responsible security reports

Please keep discussions constructive and respectful.

See [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).

---

## 📜 Licensing

Senbit's original code is licensed under the Apache License 2.0 unless explicitly stated otherwise.

The Linux kernel and third-party components retain their respective licenses.

Before contributing code originating from another project, make sure that its license is compatible with its intended use and that required attribution or notices are preserved.

By submitting a contribution, you agree that the contribution may be distributed under the applicable license of the component to which it is contributed.

---

## 🧑‍💻 Questions

If you are unsure where a change belongs, how something should be implemented, or whether an idea fits Senbit, open a GitHub Discussion.

It is better to discuss an architectural decision early than to spend days implementing something that later needs to be redesigned.

---

## 🚀 Thank you!

Whether you submit a single documentation fix or work on a major part of the operating system, every contribution helps Senbit move forward.

Thank you for helping build Senbit. 🦀🚀
