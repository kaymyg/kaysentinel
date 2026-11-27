# Help wanted

KAYSentinel is open to contributions. Below is the hardening work that would make it stronger, roughly in order of impact. Pick anything, comment on (or open) an issue so others know you are on it, and see [CONTRIBUTING.md](CONTRIBUTING.md) to get started.

Found a vulnerability instead? Please report it privately, see [SECURITY.md](SECURITY.md).


### Differential testing against more clients

**Medium.** Run the fixtures against additional execution clients and report divergences.

### Fuzz SSZ decoding

**Medium.** Add fuzz targets for the SSZ code in `runtime/ssz`.
