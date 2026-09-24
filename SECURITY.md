# Security Policy 🔐

Security is a core objective of Senbit.

As a server operating system designed for infrastructure and long-running deployments, Senbit takes security vulnerabilities seriously.

If you discover a potential vulnerability, please report it responsibly and avoid publicly disclosing the issue before it has been investigated.

---

## 🚨 Reporting a Vulnerability

**Do not report security vulnerabilities through public GitHub Issues or Discussions.**

Use GitHub's private vulnerability reporting or Security Advisories when they are available for the repository.

If private reporting is not yet available, contact the project maintainer privately through GitHub before publicly disclosing the vulnerability.

Please include as much relevant information as possible:

- A clear description of the vulnerability.
- The affected component.
- The affected version or commit.
- Steps required to reproduce the issue.
- The expected behavior.
- The actual behavior.
- The potential security impact.
- Relevant logs or diagnostic information.
- A proof of concept, when appropriate.
- A possible mitigation or fix, if you have one.

Do not include passwords, private keys, access tokens, personal information, or other secrets in a security report.

---

## 🛡️ What Should Be Reported?

Security reports may concern any part of Senbit, including:

- 🔐 Authentication and authorization
- 🧑‍💻 Privilege escalation
- 📁 Filesystem access
- 📦 Package management
- 🔄 System updates
- ↩️ Snapshot and rollback mechanisms
- 🔏 File integrity verification
- 🌐 Network services
- 🔥 Firewall configuration
- 📊 Monitoring and auditing
- 🦀 Senbit Rust components
- 🐧 Senbit-specific Linux kernel changes
- 🏗️ Build and release infrastructure
- 💾 Installation and deployment mechanisms
- ⚙️ Default system configuration

A vulnerability that exists entirely within an upstream dependency should also be reported to the appropriate upstream project when applicable.

---

## 🔎 Security Review Process

When a vulnerability is reported, the maintainers will attempt to:

1. 🔍 Reproduce the issue.
2. 🧩 Identify the affected component.
3. 📊 Determine its security impact.
4. 🛠️ Develop an appropriate fix or mitigation.
5. 🧪 Test the fix.
6. 📦 Prepare an update when necessary.
7. 📢 Publish relevant information once responsible disclosure is possible.

The exact process may evolve as Senbit develops a formal security response infrastructure.

---

## ⏱️ Responsible Disclosure

Please give the maintainers reasonable time to investigate and address a vulnerability before making technical details public.

Coordinated disclosure helps reduce the risk that an unpatched vulnerability is used against Senbit installations.

If a vulnerability affects an upstream project, disclosure may also need to be coordinated with that project's maintainers.

---

## 🔄 Security Updates

Senbit is intended to support reliable security updates as part of its update infrastructure.

The long-term system is expected to support:

- Verified updates.
- Automatic updates when configured.
- Manual updates.
- Offline updates.
- Kernel updates.
- Component-level updates.
- Snapshots before important changes.
- Rollback after failed updates.
- Update history and auditing.

These mechanisms are part of Senbit's development roadmap and should not be considered fully implemented until documented as such.

---

## 🔏 File Integrity

Senbit is designed to eventually provide mechanisms for monitoring the integrity of software installed on the system.

This may include cryptographic hashes associated with files and software components.

For example:

```text
Application
├── executable
│   └── cryptographic hash
├── libraries
│   └── cryptographic hashes
└── configuration
    └── integrity information
````

The exact implementation is still under development.

Integrity mechanisms should never be treated as a replacement for proper operating-system permissions, isolation, authentication, or other security controls.

---

## 🧱 Security by Default

Senbit aims to follow a **secure-by-default** philosophy.

A fresh installation should contain only the components required for the operating system itself.

Server applications should not be installed unnecessarily.

Administrators should be able to explicitly choose the software and services required by their deployment.

For large deployments, configuration files may eventually allow administrators to define the desired software and system configuration before installation.

---

## 📊 Monitoring and Auditing

Security is not limited to preventing attacks.

Senbit is also intended to provide visibility into the state of the system.

Future monitoring and auditing infrastructure may provide information about:

* System activity.
* Services.
* Resource usage.
* Security events.
* File integrity.
* Software changes.
* Package updates.
* System configuration.
* Administrative actions.

---

## 🧪 Supported Versions

Senbit is currently in early development and does not yet have stable releases with a formal security-support lifecycle.

Once stable releases are available, this section will document:

* Supported versions.
* Security-support periods.
* End-of-life versions.
* Security update procedures.

Until then, development versions should not be considered suitable for production environments.

---

## 📜 Scope and Third-Party Software

Senbit is built around the Linux ecosystem and will use third-party software, including the Linux kernel and Debian packages.

Not every vulnerability found in software running on Senbit is a Senbit vulnerability.

When an issue originates from an upstream project, the appropriate upstream security process should be followed.

Senbit-specific integration problems, configuration vulnerabilities, or vulnerabilities introduced by Senbit code remain within the scope of this policy.

---

## ⚠️ Production Use

Senbit is currently under active development.

Do not assume that development builds provide the security guarantees expected from a mature production operating system.

Security mechanisms described in this document may not yet exist or may still be experimental.

Always verify the documentation for the specific Senbit version being used.

---

## 🤝 Thank You

Responsible security research helps make Senbit stronger.

Thank you for taking the time to report vulnerabilities responsibly and help improve the security of the project. 🔐🦀
