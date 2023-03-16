# sudo-rs

A safety oriented and memory safe implementation in Rust of sudo and su.

## !!WARNING!!

**Sudo-rs is currently under active development and is not suited for any
production environment. Using sudo-rs is only recommended for development and
testing purposes, but you should expect any system that has sudo-rs installed to
break easily and to not be secure.**

## Quick start

Sudo-rs currently only supports Linux-based operating systems, although other
unix-based operating systems may work, they are currently not actively
supported or maintained.

Sudo-rs is written in Rust. We currently only support the latest stable compiler
toolchain. To build sudo-rs run

```
cargo build --release
```

This produces a binary `target/release/sudo`. However this binary must have the
setuid flag set and must be owned by the root user in order to provide any
useful functionality. Because we are in such an early stage we also require
an environment variable `SUDO_RS_IS_UNSTABLE` to be set, and it must have the
value `I accept that my system may break unexpectedly`. If you are unsure how
to set this value then this software is not suited for you at this time.

Sudo-rs needs the sudoers configuration file, but it currently reads it at the
`/etc/sudoers.test` location, so make sure that your sudoers configuration is at
that location. For an explanation of the sudoers syntax you can look at the
[original sudo man page](https://www.sudo.ws/docs/man/sudoers.man/). While most
syntax should be supported as is, most functionality will currently not be
implemented. Sudo-rs currently may not always warn about this, so your sudoers
file may have a different meaning compared to the original sudo implementation.

Sudo-rs always uses PAM for authentication at this time, your system must be
setup for PAM. Sudo-rs will use the sudo service configuration.

## Contributions

While we are still working on getting the basic infrastructure and architecture
of sudo-rs up and running, accepting arbitrary contributions will be difficult.
If you have any small changes or suggestions please do leave those, but if you
want to work on larger parts of sudo-rs please ask first, or we may risk doing
work twice or not being able to include your work.
