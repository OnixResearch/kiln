# Aspen runtime strict Octet gate

This generated workspace checks the production `kiln-adapter-aspen` and `kiln-aspen-extension` sources with the pinned full Octet catalog.

The shim grants only three named source exceptions. They preserve existing owner-qualified API names, production import layout, and host-sized byte contracts. It does not disable Octet lints, use a baseline, or set a warning budget.

The Nix check runs workspace, all-target, and all-feature scope with findings promoted to errors.
