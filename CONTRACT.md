# Contract - zombie-hunt

This contract is the executable spec for zombie-hunt. Every obligation below is stated so it can fail. The full contract is the first build act of this repo and it extends the stub obligations written here.

## Stub obligations

ZH-S1. Every suspect on the hunt list carries the evidence that it serves nobody, and a suspect listed without evidence is a defect the tests must catch.

ZH-S2. No kill fires before its proof is recorded, and an action taken on a suspect whose proof is missing or failed is a defect the tests must catch.

ZH-S3. The first action on any proven zombie is a reversible quarantine, and an irreversible action as the first step on any suspect is forbidden by the contract.

ZH-S4. Every hunt works from recorded or synthetic cluster state during the first slices, and no code path in those slices connects to a live cluster.

ZH-S5. Every fixture and example in this repo is synthetic and built for this repo, and no real trace or session data appears anywhere in the tree.
