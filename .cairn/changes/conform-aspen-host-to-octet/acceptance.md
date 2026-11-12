# Change-local acceptance

a[conform-aspen-host-to-octet.layout] Every source file of the Aspen host is at most 300 lines, `src/` holds at most seven library source files, no module or test filename contains an underscore, no function is longer than 70 lines without an existing allowance, and no allowance is added for file length, file count, function length or naming.

a[conform-aspen-host-to-octet.findings] The findings in the library, the binary and the two live integration tests are resolved by code changes. The only allowances added are one for the single clock read and, in the two integration tests, the `non_trait_imports` and `no_panic` allowances the older integration tests already carry. `content::MoltenContentPort` and the `gossip` items keep their public paths, and `docs::index` replaces `docs_index`.

a[conform-aspen-host-to-octet.behavior] Wire schemas, bounds, the stored content index and requirement markers are unchanged, and the host's tests pass with the same test functions as before the change.

a[conform-aspen-host-to-octet.verification] The Octet deny-all check fails at the parent revision and passes at the change, the aggregate `octet` check and the host package check pass at the change, traceability coverage passes, and the retained evidence names what was not exercised.
