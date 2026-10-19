# Security policy

Mouse Master runs as your user and can send keyboard and mouse input through uinput, so we
take security reports seriously.

Please report vulnerabilities privately through GitHub's
[private vulnerability reporting](https://github.com/burakgon/mouse-master-linux/security/advisories/new)
rather than in a public issue. We aim to acknowledge reports within a few days.

Things we consider in scope:

- ways for another local user to control or read input through Mouse Master,
- the udev rules granting more access than intended (they use `uaccess`, so only the user of
  the active seat gets access),
- the D-Bus API allowing actions it should not (for example arbitrary commands from a
  sandboxed application),
- configuration parsing that can crash or misbehave on crafted files.
